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
/// zerescobblemon.holy.gg -> de04.holy.gg:19556). Si no hay SRV, se queda host:puerto.
pub async fn resolve(host: &str, port: u16) -> (String, u16) {
    if port != 25565 {
        return (host.to_string(), port);
    }
    let lookup = async {
        let resolver = hickory_resolver::TokioAsyncResolver::tokio_from_system_conf().ok()?;
        let srv = resolver.srv_lookup(format!("_minecraft._tcp.{host}.")).await.ok()?;
        let rec = srv.iter().min_by_key(|r| (r.priority(), u16::MAX - r.weight()))?;
        Some((rec.target().to_utf8().trim_end_matches('.').to_string(), rec.port()))
    };
    tokio::time::timeout(Duration::from_secs(3), lookup).await.ok().flatten().unwrap_or((host.to_string(), port))
}

pub async fn status(host: &str, port: u16) -> Result<(Value, u64)> {
    let (host, port) = resolve(host, port).await;
    let host = host.as_str();
    let start = Instant::now();
    let mut s = tokio::time::timeout(Duration::from_secs(4), TcpStream::connect((host, port))).await??;
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
