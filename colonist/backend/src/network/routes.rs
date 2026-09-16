use actix_web::{web, HttpRequest, HttpResponse, Error, get};
use actix_web_actors::ws;
use actix::Addr;
use log::info;
use serde::Deserialize;
use uuid::Uuid;
use crate::lobby::Lobby;
use crate::network::message::Authenticate;
use crate::network::ws_worker::WsWorker;

#[derive(Deserialize)]
pub struct AuthQuery {
    /// Session token from a previous connection, if the client has one.
    pub token: Option<Uuid>,
}

#[get("/ws")]
pub async fn ws_index(req: HttpRequest, stream: web::Payload, lobby_addr: web::Data<Addr<Lobby>>, query: web::Query<AuthQuery>) -> Result<HttpResponse, Error> {
    // Identity is never taken from the client: the lobby resolves the token,
    // or mints a brand new player if the token is missing or unknown.
    let auth = lobby_addr
        .send(Authenticate { token: query.token })
        .await
        .map_err(|_| actix_web::error::ErrorServiceUnavailable("lobby unavailable"))?;

    info!("player_id: {}", auth.player_id);

    let worker = WsWorker::new(auth.player_id, auth.token, lobby_addr.get_ref().clone());
    ws::start(worker, &req, stream)
}
