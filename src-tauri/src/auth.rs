//! Cuentas: sin premium (UUID offline, como el servidor) y Microsoft (codigo de dispositivo -> Xbox -> Minecraft).

use crate::net;
use crate::settings::Account;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

/// Client ID de la app de Azure de Zeres. Mojang tiene que aprobarla para que funcione el paso de Minecraft;
/// mientras este vacio, el inicio de sesion con Microsoft queda desactivado en la beta.
pub const MS_CLIENT_ID: &str = "";

/// El mismo UUID que da el servidor en modo offline: MD5("OfflinePlayer:" + nombre), version 3.
pub fn offline_uuid(name: &str) -> String {
    let digest = md5::compute(format!("OfflinePlayer:{name}").as_bytes());
    uuid::Builder::from_md5_bytes(digest.0).into_uuid().simple().to_string()
}

pub fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn offline(name: &str) -> Account {
    Account {
        kind: "offline".into(),
        name: name.into(),
        uuid: offline_uuid(name),
        access_token: "0".into(),
        refresh_token: String::new(),
        expires_at: 0,
        skin_url: String::new(),
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ------------------------------------------------------------------ Microsoft

pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

pub async fn device_code() -> Result<DeviceCode> {
    if MS_CLIENT_ID.is_empty() {
        return Err(anyhow!("El inicio de sesión con Microsoft aún no está disponible en esta beta (falta la aprobación de Mojang)."));
    }
    let r: Value = net::client()
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[("client_id", MS_CLIENT_ID), ("scope", "XboxLive.signin offline_access")])
        .send()
        .await?
        .json()
        .await?;
    Ok(DeviceCode {
        device_code: r["device_code"].as_str().unwrap_or_default().into(),
        user_code: r["user_code"].as_str().unwrap_or_default().into(),
        verification_uri: r["verification_uri"].as_str().unwrap_or("https://www.microsoft.com/link").into(),
        interval: r["interval"].as_u64().unwrap_or(5),
        expires_in: r["expires_in"].as_u64().unwrap_or(900),
    })
}

/// Espera a que el jugador meta el codigo; devuelve (access_token, refresh_token) de Microsoft.
pub async fn poll_token(dc: &DeviceCode) -> Result<(String, String)> {
    let deadline = now() + dc.expires_in;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(dc.interval.max(2))).await;
        if now() > deadline {
            return Err(anyhow!("El código ha caducado. Vuelve a intentarlo."));
        }
        let r: Value = net::client()
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", MS_CLIENT_ID),
                ("device_code", &dc.device_code),
            ])
            .send()
            .await?
            .json()
            .await?;
        if let Some(t) = r["access_token"].as_str() {
            return Ok((t.into(), r["refresh_token"].as_str().unwrap_or_default().into()));
        }
        match r["error"].as_str() {
            Some("authorization_pending") | Some("slow_down") => continue,
            Some(e) => return Err(anyhow!("Microsoft: {e}")),
            None => return Err(anyhow!("respuesta rara de Microsoft")),
        }
    }
}

async fn refresh_ms(refresh: &str) -> Result<(String, String)> {
    let r: Value = net::client()
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[("grant_type", "refresh_token"), ("client_id", MS_CLIENT_ID), ("refresh_token", refresh), ("scope", "XboxLive.signin offline_access")])
        .send()
        .await?
        .json()
        .await?;
    let t = r["access_token"].as_str().ok_or_else(|| anyhow!("Hay que volver a iniciar sesión con Microsoft"))?;
    Ok((t.into(), r["refresh_token"].as_str().unwrap_or(refresh).into()))
}

/// Microsoft -> Xbox Live -> XSTS -> Minecraft -> perfil.
pub async fn minecraft_login(ms_token: &str, refresh: &str) -> Result<Account> {
    let c = net::client();
    let xbl: Value = c
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({"Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_token}")},
                      "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT"}))
        .send().await?.json().await.context("Xbox Live")?;
    let xbl_token = xbl["Token"].as_str().ok_or_else(|| anyhow!("Xbox Live no respondió"))?;
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().unwrap_or_default();
    let xsts: Value = c
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({"Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
                      "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT"}))
        .send().await?.json().await.context("XSTS")?;
    let Some(xsts_token) = xsts["Token"].as_str() else {
        return Err(match xsts["XErr"].as_u64() {
            Some(2148916233) => anyhow!("Esa cuenta de Microsoft no tiene perfil de Xbox."),
            Some(2148916238) => anyhow!("La cuenta es de un menor: tiene que estar en una familia de Microsoft."),
            _ => anyhow!("Xbox no dejó iniciar sesión."),
        });
    };
    let mc: Value = c
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({"identityToken": format!("XBL3.0 x={uhs};{xsts_token}")}))
        .send().await?.json().await.context("Minecraft")?;
    let access = mc["access_token"].as_str().ok_or_else(|| anyhow!("Minecraft rechazó el inicio de sesión (¿app de Azure sin aprobar?)"))?;
    let expires = now() + mc["expires_in"].as_u64().unwrap_or(86400);
    let profile: Value = c.get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(access).send().await?.json().await?;
    let name = profile["name"].as_str().ok_or_else(|| anyhow!("Esta cuenta no tiene Minecraft comprado."))?;
    let skin = profile["skins"].as_array().and_then(|s| s.iter().find(|x| x["state"] == "ACTIVE")).and_then(|s| s["url"].as_str()).unwrap_or_default();
    Ok(Account {
        kind: "microsoft".into(),
        name: name.into(),
        uuid: profile["id"].as_str().unwrap_or_default().into(),
        access_token: access.into(),
        refresh_token: refresh.into(),
        expires_at: expires,
        skin_url: skin.into(),
    })
}

/// Renueva el token de Minecraft si va a caducar.
pub async fn ensure_fresh(acc: &Account) -> Result<Account> {
    if acc.kind != "microsoft" || acc.expires_at > now() + 300 {
        return Ok(acc.clone());
    }
    let (ms, refresh) = refresh_ms(&acc.refresh_token).await?;
    minecraft_login(&ms, &refresh).await
}
