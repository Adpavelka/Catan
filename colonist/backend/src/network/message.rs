use actix::prelude::*;
use uuid::Uuid;
use shared::ClientRequest;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub addr: Recipient<ServerMessage>,
    pub player_id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub player_id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientActorMessage {
    pub player_id: Uuid,
    pub req: ClientRequest,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct ServerMessage(pub String); 
/// Resolve a client-supplied session token to a player identity, minting a new
/// one when the client has no valid token.
#[derive(Message)]
#[rtype(result = "Authenticated")]
pub struct Authenticate {
    pub token: Option<Uuid>,
}

pub struct Authenticated {
    pub player_id: Uuid,
    pub token: Uuid,
}
