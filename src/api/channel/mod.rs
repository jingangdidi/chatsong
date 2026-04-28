use crate::error::MyError;

pub mod discord;

pub enum Channel {
    Discord{token: String, guild_id: u64},
}

impl Channel {
    /// create Channel from string
    pub fn new_from_str(channel: &str) -> Result<Self, MyError> {
        let tmp: Vec<&str> = channel.split(":").collect();
        match tmp[0].to_lowercase().as_ref() {
            "discord" => {
                if tmp.len() == 3 {
                    let guild_id = match tmp[2].parse::<u64>() {
                        Ok(i) => i,
                        Err(e) => return Err(MyError::ParaError{para: format!("cannot parse guild id \"{}\" as u64: {:?}", tmp[2], e)}),
                    };
                    Ok(Channel::Discord{token: tmp[1].to_string(), guild_id})
                } else {
                    Err(MyError::ParaError{para: format!("discord channel must specify bot-token and guild-id, not \"{}\"", channel)})
                }
            },
            _ => Err(MyError::ParaError{para: format!("channel only support: discord, not \"{}\"", tmp[0])}),
        }
    }

    /// start bot
    pub async fn start_bot(&self) {
        match self {
            Channel::Discord{token, ..} => discord::run_discord_bot(&token).await
        }
    }
}
