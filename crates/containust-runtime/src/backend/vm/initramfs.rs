//! Custom initramfs builder for the Containust VM agent.
//!
//! Takes the stock Alpine Linux initramfs, unpacks it, injects
//! a custom init script and the Containust agent, then repacks
//! it as a gzip-compressed cpio archive.

use std::io::{Read, Write};
use std::path::Path;

use containust_common::error::{ContainustError, Result};

/// PID 1 init script. Creates all directories, installs busybox symlinks,
/// mounts filesystems, sets up networking, and execs the agent.
const INIT_SCRIPT: &str = r#"#!/bin/sh
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

/bin/busybox mkdir -p /usr/sbin /usr/bin /usr/local/bin /proc /sys /run /tmp /var /root
/bin/busybox --install -s 2>/dev/null

mount -t proc proc /proc
mount -t sysfs sys /sys
mount -t devtmpfs dev /dev
mkdir -p /dev/pts /dev/shm
mount -t devpts devpts /dev/pts
mount -t tmpfs tmpfs /dev/shm
mount -t tmpfs tmpfs /run
mount -t tmpfs tmpfs /tmp
chmod 1777 /tmp

hostname containust-vm

modprobe -q virtio_net 2>/dev/null
modprobe -q virtio_pci 2>/dev/null
modprobe -q virtio_blk 2>/dev/null

ip link set lo up
for iface in eth0 enp0s1 ens3; do
    ip link set "$iface" up 2>/dev/null && break
done

if command -v udhcpc >/dev/null 2>&1; then
    udhcpc -i eth0 -s /usr/share/udhcpc/default.script -q -n -t 5 2>/dev/null
fi
ip route add default via 10.0.2.2 2>/dev/null
echo "nameserver 10.0.2.3" > /etc/resolv.conf

mkdir -p /tmp/containust/containers /tmp/containust/logs /tmp/containust/rootfs

exec /sbin/containust-agent
"#;

/// The Containust agent bootstrap. Creates a handler script at /tmp/handler.sh
/// then launches nc in a loop with -e to spawn a new handler per connection.
/// The handler script contains ALL container lifecycle logic and reads one
/// JSON-RPC line from stdin, processes it, writes the response to stdout.
const AGENT_SCRIPT: &str = r##"#!/bin/sh
PORT=10809
SD="/tmp/containust/containers"
LD="/tmp/containust/logs"
RD="/tmp/containust/rootfs"
mkdir -p "$SD" "$LD" "$RD"

# Write standalone handler that nc -e invokes per connection
cat > /tmp/handler.sh << 'HANDLER_EOF'
#!/bin/sh
SD="/tmp/containust/containers"
LD="/tmp/containust/logs"
RD="/tmp/containust/rootfs"

gen_id() { cat /proc/sys/kernel/random/uuid 2>/dev/null | tr -d '-' | head -c 16; }

h_create() {
    local id=$(gen_id)
    local nm=$(echo "$1"|sed -n 's/.*"name" *: *"\([^"]*\)".*/\1/p')
    local im=$(echo "$1"|sed -n 's/.*"image" *: *"\([^"]*\)".*/\1/p')
    local pt=$(echo "$1"|sed -n 's/.*"port" *: *\([0-9][0-9]*\).*/\1/p')
    local cm=$(echo "$1"|sed -n 's/.*"command" *: *\(\[[^]]*\]\).*/\1/p')
    [ -z "$cm" ] && cm='["sh"]'
    mkdir -p "$SD/$id"
    echo "{\"id\":\"$id\",\"name\":\"$nm\",\"image\":\"$im\",\"port\":\"$pt\",\"command\":$cm,\"state\":\"created\",\"created_at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}" > "$SD/$id/meta.json"
    local r="$RD/$id"
    mkdir -p "$r/bin" "$r/sbin" "$r/usr/bin" "$r/usr/sbin" "$r/usr/local/bin" "$r/lib" "$r/etc" "$r/proc" "$r/sys" "$r/dev" "$r/tmp" "$r/var" "$r/root" "$r/home" "$r/run"
    cp -a /lib/ld-musl-* "$r/lib/" 2>/dev/null
    cp -a /lib/libc.musl-* "$r/lib/" 2>/dev/null
    if [ -f /bin/busybox ]; then
        cp /bin/busybox "$r/bin/"
        chroot "$r" /bin/busybox --install -s 2>/dev/null
        [ ! -e "$r/bin/sh" ] && ln -s busybox "$r/bin/sh"
    fi
    echo "nameserver 10.0.2.3" > "$r/etc/resolv.conf"
    echo "{\"result\":{\"id\":\"$id\"}}"
}

h_start() {
    local id=$(echo "$1"|sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p')
    [ ! -d "$SD/$id" ] && echo "{\"error\":\"not found: $id\"}" && return
    local r="$RD/$id"
    local lf="$LD/$id.log"
    # Extract command array and write as a runnable shell script
    local cm=$(sed -n 's/.*"command":\(\[[^]]*\]\).*/\1/p' "$SD/$id/meta.json")
    # Handle ["sh","-c","actual command"] pattern
    local third=$(echo "$cm" | sed -n 's/\[[^,]*,[^,]*,"\(.*\)"\]/\1/p')
    if [ -n "$third" ]; then
        echo "#!/bin/sh" > "$r/tmp/run.sh"
        echo "$third" >> "$r/tmp/run.sh"
        chmod 755 "$r/tmp/run.sh"
    else
        local sc=$(echo "$cm"|sed 's/^\[//;s/\]$//;s/","/ /g;s/"//g')
        [ -z "$sc" ] && sc="sh"
        echo "#!/bin/sh" > "$r/tmp/run.sh"
        echo "exec $sc" >> "$r/tmp/run.sh"
        chmod 755 "$r/tmp/run.sh"
    fi
    mount -t proc proc "$r/proc" 2>/dev/null
    mount --bind /dev "$r/dev" 2>/dev/null
    chroot "$r" /bin/sh /tmp/run.sh >"$lf" 2>&1 &
    local p=$!
    echo "$p" > "$SD/$id/pid"
    sed -i 's/"state":"[^"]*"/"state":"running"/' "$SD/$id/meta.json"
    echo "{\"result\":{\"pid\":$p}}"
}

h_stop() {
    local id=$(echo "$1"|sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p')
    [ ! -d "$SD/$id" ] && echo "{\"error\":\"not found: $id\"}" && return
    [ -f "$SD/$id/pid" ] && { kill $(cat "$SD/$id/pid") 2>/dev/null; sleep 1; kill -9 $(cat "$SD/$id/pid") 2>/dev/null; rm "$SD/$id/pid"; }
    local r="$RD/$id"
    umount "$r/dev" 2>/dev/null; umount "$r/proc" 2>/dev/null
    sed -i 's/"state":"[^"]*"/"state":"stopped"/' "$SD/$id/meta.json"
    echo '{"result":"ok"}'
}

h_exec() {
    local id=$(echo "$1"|sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p')
    [ ! -d "$SD/$id" ] && echo "{\"error\":\"not found: $id\"}" && return
    local cm=$(echo "$1"|sed -n 's/.*"command" *: *\(\[[^]]*\]\).*/\1/p')
    local sc=$(echo "$cm"|sed 's/^\[//;s/\]$//;s/","/ /g;s/"//g')
    local r="$RD/$id"
    local o=$(chroot "$r" /bin/sh -c "$sc" 2>/tmp/e.$id)
    local rc=$?
    local e=$(cat /tmp/e.$id 2>/dev/null); rm -f /tmp/e.$id
    o=$(printf '%s' "$o"|sed 's/"/\\"/g'|tr '\n' ' ')
    e=$(printf '%s' "$e"|sed 's/"/\\"/g'|tr '\n' ' ')
    echo "{\"result\":{\"stdout\":\"$o\",\"stderr\":\"$e\",\"exit_code\":$rc}}"
}

h_logs() {
    local id=$(echo "$1"|sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p')
    local lf="$LD/$id.log"
    if [ -f "$lf" ]; then
        local c=$(cat "$lf"|sed 's/"/\\"/g'|tr '\n' ' ')
        echo "{\"result\":{\"logs\":\"$c\"}}"
    else
        echo '{"result":{"logs":""}}'
    fi
}

h_list() {
    local res='{"result":{"containers":['
    local f=1
    for dd in "$SD"/*/; do
        [ -f "$dd/meta.json" ] || continue
        [ "$f" = 0 ] && res="$res,"
        f=0
        res="$res$(cat "$dd/meta.json")"
    done
    echo "$res]}}"
}

h_remove() {
    local id=$(echo "$1"|sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p')
    h_stop "$1" >/dev/null 2>&1
    rm -rf "$SD/$id" "$RD/$id" "$LD/$id.log"
    echo '{"result":"ok"}'
}

read -r line
m=$(echo "$line" | sed -n 's/.*"method" *: *"\([^"]*\)".*/\1/p')
case "$m" in
    ping) echo '{"result":"pong"}';;
    create) h_create "$line";;
    start) h_start "$line";;
    stop) h_stop "$line";;
    exec) h_exec "$line";;
    logs) h_logs "$line";;
    list) h_list;;
    remove) h_remove "$line";;
    *) echo "{\"error\":\"unknown: $m\"}";;
esac
HANDLER_EOF
chmod 755 /tmp/handler.sh

echo "containust-agent: listening on port $PORT"
while true; do
    nc -ll -p "$PORT" -e /tmp/handler.sh 2>/dev/null
    nc -l -p "$PORT" -e /tmp/handler.sh 2>/dev/null
    sleep 0.1
done
"##;

/// Builds a custom initramfs by unpacking the Alpine base, injecting
/// directory entries, the Containust init and agent scripts, and repacking.
///
/// # Errors
///
/// Returns an error if the base initramfs cannot be read, decompressed,
/// or the output cannot be written.
pub fn build_initramfs(base_initramfs: &Path, output: &Path) -> Result<()> {
    let base_data = std::fs::read(base_initramfs).map_err(|e| ContainustError::Io {
        path: base_initramfs.to_path_buf(),
        source: e,
    })?;

    let output_file = std::fs::File::create(output).map_err(|e| ContainustError::Io {
        path: output.to_path_buf(),
        source: e,
    })?;

    let gz_encoder = flate2::write::GzEncoder::new(output_file, flate2::Compression::fast());
    let mut cpio = CpioWriter::new(gz_encoder);

    unpack_and_repack_base(&base_data, &mut cpio)?;

    for dir in &["tmp", "run", "var", "root", "proc", "sys", "dev"] {
        cpio.write_dir(dir)?;
    }

    cpio.write_entry("init", 0o100_755, INIT_SCRIPT.as_bytes())?;
    cpio.write_entry("sbin/containust-init", 0o100_755, INIT_SCRIPT.as_bytes())?;
    cpio.write_entry("sbin/containust-agent", 0o100_755, AGENT_SCRIPT.as_bytes())?;

    cpio.write_trailer()?;

    let gz = cpio.finish();
    let _ = gz.finish().map_err(|e| ContainustError::Io {
        path: output.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// Unpacks the gzip-compressed cpio base initramfs and writes all
/// entries into the new cpio archive.
fn unpack_and_repack_base<W: Write>(data: &[u8], writer: &mut CpioWriter<W>) -> Result<()> {
    let decoder = flate2::read::GzDecoder::new(data);
    let mut reader = CpioReader::new(decoder);

    while let Some(entry) = reader.next_entry()? {
        if entry.name == "TRAILER!!!" {
            break;
        }
        if entry.name == "init"
            || entry.name == "sbin/containust-init"
            || entry.name == "sbin/containust-agent"
        {
            continue;
        }
        writer.write_entry(&entry.name, entry.mode, &entry.data)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CPIO newc format writer
// ---------------------------------------------------------------------------

struct CpioWriter<W: Write> {
    inner: W,
    ino: u32,
    pos: usize,
}

impl<W: Write> CpioWriter<W> {
    const fn new(inner: W) -> Self {
        Self {
            inner,
            ino: 300_000,
            pos: 0,
        }
    }

    /// Writes a directory entry (mode 040755, zero data).
    fn write_dir(&mut self, name: &str) -> Result<()> {
        self.write_entry(name, 0o040_755, &[])
    }

    fn write_entry(&mut self, name: &str, mode: u32, data: &[u8]) -> Result<()> {
        self.ino += 1;
        let name_nul = format!("{name}\0");
        let namesize = name_nul.len();
        let filesize = data.len();

        let header = format!(
            "070701\
             {:08X}{:08X}{:08X}{:08X}\
             {:08X}{:08X}{:08X}{:08X}\
             {:08X}{:08X}{:08X}{:08X}\
             {:08X}",
            self.ino, mode, 0u32, 0u32,
            1u32, 0u32, filesize, 0u32,
            0u32, 0u32, 0u32, namesize,
            0u32,
        );

        self.raw_write(header.as_bytes())?;
        self.raw_write(name_nul.as_bytes())?;
        self.align4()?;
        self.raw_write(data)?;
        self.align4()?;

        Ok(())
    }

    fn write_trailer(&mut self) -> Result<()> {
        self.write_entry("TRAILER!!!", 0, &[])
    }

    fn finish(self) -> W {
        self.inner
    }

    fn raw_write(&mut self, buf: &[u8]) -> Result<()> {
        self.inner.write_all(buf).map_err(|e| ContainustError::Io {
            path: std::path::PathBuf::from("<cpio>"),
            source: e,
        })?;
        self.pos += buf.len();
        Ok(())
    }

    fn align4(&mut self) -> Result<()> {
        let pad = (4 - (self.pos % 4)) % 4;
        if pad > 0 {
            self.raw_write(&vec![0u8; pad])?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CPIO newc format reader
// ---------------------------------------------------------------------------

struct CpioEntry {
    name: String,
    mode: u32,
    data: Vec<u8>,
}

struct CpioReader<R: Read> {
    inner: R,
}

impl<R: Read> CpioReader<R> {
    const fn new(inner: R) -> Self {
        Self { inner }
    }

    fn skip_padding(&mut self, offset: usize) {
        let pad = (4 - (offset % 4)) % 4;
        if pad > 0 {
            let mut buf = vec![0u8; pad];
            let _ = self.inner.read_exact(&mut buf);
        }
    }

    fn read_exact_cpio(&mut self, buf: &mut [u8]) -> Result<()> {
        self.inner.read_exact(buf).map_err(|e| ContainustError::Io {
            path: std::path::PathBuf::from("<cpio>"),
            source: e,
        })
    }

    fn next_entry(&mut self) -> Result<Option<CpioEntry>> {
        let mut header = [0u8; 110];
        match self.inner.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => {
                return Err(ContainustError::Io {
                    path: std::path::PathBuf::from("<cpio>"),
                    source: e,
                });
            }
        }

        let magic = std::str::from_utf8(&header[..6]).unwrap_or("");
        if magic != "070701" {
            return Err(ContainustError::Config {
                message: format!("invalid CPIO magic: {magic}"),
            });
        }

        let mode = parse_hex(&header[14..22]);
        let filesize = parse_hex(&header[54..62]) as usize;
        let namesize = parse_hex(&header[94..102]) as usize;

        let mut name_buf = vec![0u8; namesize];
        self.read_exact_cpio(&mut name_buf)?;
        let name = String::from_utf8_lossy(&name_buf).trim_end_matches('\0').to_string();
        self.skip_padding(110 + namesize);

        let mut data = vec![0u8; filesize];
        if filesize > 0 {
            self.read_exact_cpio(&mut data)?;
        }
        self.skip_padding(filesize);

        Ok(Some(CpioEntry { name, mode, data }))
    }
}

fn parse_hex(bytes: &[u8]) -> u32 {
    let s = std::str::from_utf8(bytes).unwrap_or("0");
    u32::from_str_radix(s, 16).unwrap_or(0)
}
