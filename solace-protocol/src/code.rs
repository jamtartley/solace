// @TODO: Rethink this system
pub const RES_ACK_MESSAGE: u16 = 0;
pub const RES_WELCOME: u16 = 1;
pub const RES_YOUR_NICK: u16 = 2;
pub const RES_HELLO: u16 = 3;
pub const RES_GOODBYE: u16 = 4;
pub const RES_PONG: u16 = 5;
pub const RES_DISCONNECTED: u16 = 6;

pub const RES_CHAT_MESSAGE_OK: u16 = 200;
pub const RES_NICK_CHANGE: u16 = 201;
pub const RES_TOPIC_CHANGE: u16 = 202;
pub const RES_TOPIC_CHANGE_MESSAGE: u16 = 203;
pub const RES_COMMAND_LIST: u16 = 204;
pub const RES_NICK_LIST: u16 = 205;
pub const RES_WHO_IS: u16 = 206;

pub const ERR_COMMAND_NOT_FOUND: u16 = 300;
pub const ERR_INVALID_ARGUMENT: u16 = 301;
pub const ERR_NICK_IN_USE: u16 = 302;
pub const ERR_WHO_IS: u16 = 303;
