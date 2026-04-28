use serenity::builder::{CreateCommand, CreateCommandOption};
use serenity::model::application::{CommandOptionType, ResolvedOption, ResolvedValue};

/// 输入`/`会自动显示出所有命令，点击`/id`，此时输入框会自动写入`/id id:`，且输入框上方显示所有用户，点击其中一个用户，此时输入框自动加上了该用户名`/id id:@jingangdidi`
pub fn run(options: &[ResolvedOption]) -> String {
    if let Some(ResolvedOption { value: ResolvedValue::User(user, _), .. }) = options.first() {
        format!("{}'s id is {}", user.tag(), user.id)
    } else {
        "Please provide a valid user".to_string()
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("id").description("Get a user id").add_option(
        CreateCommandOption::new(CommandOptionType::User, "id", "The user to lookup").required(true),
    )
}
