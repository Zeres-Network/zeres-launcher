//! "Server List Ping": jugadores conectados, MOTD e icono del servidor, como la lista de servidores del juego.

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn varint(mut v: i32, out: &mut Vec<u8>) {
    loop {
        let mut b = (v & 0x7F) as u8;
        v = ((v as u32) >> 7) as i32;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

async fn read_varint(s: &mut TcpStream) -> Result<i32> {
    let mut value = 0i32;
    for i in 0..5 {
        let b = s.read_u8().await?;
        value |= ((b & 0x7F) as i32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(anyhow!("VarInt demasiado largo"))
}

/// Como hace Minecraft: si no se da puerto, mira el registro SRV `_minecraft._tcp.<host>` (p. ej.
/// zerescobblemon.holy.gg -> de04.holy.gg:19556). Si no hay SRV, se queda host:puerto. Prueba primero con el DNS del
/// sistema y, si falla, con el de Cloudflare.
pub async fn resolve(host: &str, port: u16) -> (String, u16) {
    if port != 25565 || host.parse::<std::net::IpAddr>().is_ok() {
        return (host.to_string(), port);
    }
    use hickory_resolver::config::{ResolverConfig, ResolverOpts};
    use hickory_resolver::TokioAsyncResolver;
    let name = format!("_minecraft._tcp.{host}.");
    let resolvers = [
        TokioAsyncResolver::tokio_from_system_conf().ok(),
        Some(TokioAsyncResolver::tokio(ResolverConfig::cloudflare(), ResolverOpts::default())),
    ];
    for r in resolvers.into_iter().flatten() {
        if let Ok(Ok(srv)) = tokio::time::timeout(Duration::from_secs(3), r.srv_lookup(name.clone())).await {
            if let Some(rec) = srv.iter().min_by_key(|r| (r.priority(), u16::MAX - r.weight())) {
                return (rec.target().to_utf8().trim_end_matches('.').to_string(), rec.port());
            }
        }
    }
    (host.to_string(), port)
}

/// Estado del servidor. Si el destino de la redireccion no responde, prueba la direccion tal cual.
pub async fn status(host: &str, port: u16) -> Result<(Value, u64)> {
    let (rh, rp) = resolve(host, port).await;
    match status_at(&rh, rp).await {
        Ok(v) => Ok(v),
        Err(e) if rh != host || rp != port => status_at(host, port).await.map_err(|e2| anyhow!("{rh}:{rp}: {e} / {host}:{port}: {e2}")),
        Err(e) => Err(anyhow!("{host}:{port}: {e}")),
    }
}

async fn status_at(host: &str, port: u16) -> Result<(Value, u64)> {
    let start = Instant::now();
    let mut s = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect((host, port))).await??;
    let mut hs = Vec::new();
    varint(0, &mut hs); // id del paquete
    varint(767, &mut hs); // protocolo 1.21.1
    varint(host.len() as i32, &mut hs);
    hs.extend_from_slice(host.as_bytes());
    hs.extend_from_slice(&port.to_be_bytes());
    varint(1, &mut hs); // siguiente estado: status
    let mut pkt = Vec::new();
    varint(hs.len() as i32, &mut pkt);
    pkt.extend(hs);
    pkt.extend_from_slice(&[1, 0]); // peticion de status
    s.write_all(&pkt).await?;
    let read = async {
        let _len = read_varint(&mut s).await?;
        let _id = read_varint(&mut s).await?;
        let n = read_varint(&mut s).await? as usize;
        let mut buf = vec![0u8; n];
        s.read_exact(&mut buf).await?;
        Ok::<_, anyhow::Error>(serde_json::from_slice::<Value>(&buf)?)
    };
    let v = tokio::time::timeout(Duration::from_secs(5), read).await??;
    Ok((v, start.elapsed().as_millis() as u64))
}
