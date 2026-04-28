#![allow(deprecated)]

use std::collections::HashSet;

use serenity::async_trait;
use serenity::builder::EditChannel;
use serenity::framework::standard::buckets::LimitedFor;
use serenity::framework::standard::macros::{check, command, group, help, hook};
use serenity::framework::standard::{
    help_commands,
    Args,
    BucketBuilder,
    CommandGroup,
    CommandOptions,
    CommandResult,
    Configuration,
    DispatchError,
    HelpOptions,
    Reason,
    StandardFramework,
};
//use serenity::gateway::ShardManager;
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::UserId;
//use serenity::model::permissions::Permissions;
use serenity::prelude::*;
use serenity::utils::MessageBuilder;




use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::model::application::{Command, Interaction};
use serenity::model::id::GuildId;

use tracing::{event, Level};

use crate::{
    parse_paras::PARAS,
    channel::Channel,
};

mod commands;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    async fn ready(&self, ctx: Context, ready: Ready) {
        event!(Level::INFO, "{} is connected!", ready.user.name);

        /*
        let guild_id = GuildId::new(
            env::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );
        */
        // 这个id不是app-id，手机`您 --> 右上角设置 --> 高级设置 --> 开启"开发者模式" --> 返回主页 --> 按住自己的server头像 --> 更多选项 --> 划到最后"复制服务器ID"
        let discord_guild_id: u64 = PARAS.channels
            .iter()
            .find(|channel| matches!(channel, Channel::Discord{ .. }))
            .and_then(|channel| match channel {
                Channel::Discord{ guild_id, .. } => Some(*guild_id),
            })
            .unwrap_or_else(|| 0);
        let guild_id = GuildId::new(discord_guild_id);

        // 这些命令设置在自己的服务器内，只能在该服务器内调用
        let _commands = guild_id
            .set_commands(&ctx.http, vec![
                commands::ping::register(),
                commands::id::register(),
                commands::welcome::register(), // 这里设置了这个命令，即可以在输入框调用，但是上面没有解析该命令进行调用，回复`not implemented :(`
                commands::numberinput::register(), // 这里设置了这个命令，即可以在输入框调用，但是上面没有解析该命令进行调用，回复`not implemented :(`
                commands::attachmentinput::register(),
                commands::modal::register(),
            ])
            .await;
        //event!(Level::INFO, "I now have the following guild slash commands: {:#?}", commands);

        // 这个命令设置为全局可调用，在服务器内以及私信中均可调用
        let _global_command = Command::create_global_command(&ctx.http, commands::wonderful_command::register()).await;
        //event!(Level::INFO, "I created the following global slash command: {:#?}", global_command);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            //println!("Received command interaction: {command:#?}");

            let content = match command.data.name.as_str() {
                "ping" => Some(commands::ping::run(&command.data.options())),
                "id" => Some(commands::id::run(&command.data.options())),
                "attachmentinput" => Some(commands::attachmentinput::run(&command.data.options())),
                "modal" => {
                    commands::modal::run(&ctx, &command).await.unwrap();
                    None
                },
                _ => Some("not implemented :(".to_string()),
            };

            if let Some(content) = content {
                let data = CreateInteractionResponseMessage::new().content(content);
                let builder = CreateInteractionResponse::Message(data);
                if let Err(e) = command.create_response(&ctx.http, builder).await {
                    event!(parent: None, Level::ERROR, "Cannot respond to slash command: {}", e);
                }
            }
        }
    }

    // Set a handler for the `message` event. This is called whenever a new message is received.
    //
    // Event handlers are dispatched through a threadpool, and so multiple events can be
    // dispatched simultaneously.
    // 这个是监听群里消息和bot收到的私信，如果用户在群里发信息，则bot在群里回复，如果用户给bot发私信，则bot直接在私信回复，不是回复在群里
    async fn message(&self, ctx: Context, msg: Message) {
        //println!("shard id: {}, user: {}", ctx.shard_id, msg.content);
        // ignore Bot message
        if msg.author.bot {
            return
        }
        if msg.content == "!ping" {
            let tmp_code = r#"
fn main() {
    println!("Hello, world!");
}
"#;
            // The message builder allows for creating a message by mentioning users dynamically,
            // pushing "safe" versions of content (such as bolding normalized content), displaying
            // emojis, and more.
            // 这里可以字体加粗、提及、使用emoji、代码块、斜体、下划线等
            // https://docs.rs/serenity/0.12.5/serenity/utils/struct.MessageBuilder.html
            let response = MessageBuilder::new()
                .push("User ")
                .push_bold_safe(&msg.author.name)
                .push(" used the 'ping' command in the ")
                .push(" channel😘\n")
                .push_line_safe("新行") // 新行
                .push_bold_line_safe("加粗的新行") // 整行加粗
                .push_codeblock_safe(tmp_code, Some("rust")) // 代码块
                .push_italic_line_safe("italic") // 斜体，中文无效
                .push_mono_line_safe("等宽字符")
                .push_mono_line_safe("monospaced text") // 等宽字体
                .push_line_safe("normal text")
                .push_line_safe("") // 空行
                .push_quote_line_safe("quote line") // 标注引用
                .push_line_safe("") // 空行
                .push_spoiler_line_safe("@everyone") // 盖住内容，点击才显示
                .push_strike_line_safe("划掉") // 划掉
                .push_underline_line_safe("下划线") // 下划线
                .build();

            if let Err(e) = msg.channel_id.say(&ctx.http, &response).await {
                event!(parent: None, Level::ERROR, "Error sending message: {:?}", e);
            }
        }
    }
}

// ------------------------------general------------------------------
// 定义常规命令：
// 输入`~about`，bot回复`This is a small test-bot! : )`
// 输入`~ping`，bot回复`Pong! : )`
// 输入`~upper`，bot回复`This is the main command!`
// 输入`~upper sub`或，`~upper secret`，bot回复`This is a sub command!`
#[group]
#[commands(about, ping, upper_command)]
#[description = "A group with commands."] // Set a description to appear if a user wants to display a single group e.g. via help using the group-name or one of its prefixes.
#[summary = "group commands"] // Summary only appears when listing multiple groups.
struct General;

// 显示该bot的信息
#[command]
#[bucket = "about"]
#[description("Bot info.")]
async fn about(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, "This is my bot! : )").await?;
    Ok(())
}

#[command]
#[only_in(guilds)] // 仅在服务器中可用，私信无效
#[checks(Owner)] // 指定的所有者才能调用该命令
#[required_permissions("ADMINISTRATOR")] // Allow only administrators to call this
#[description("ping-pong")]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, "Pong! : )").await?;
    Ok(())
}

// 至输入`~upper`，则回复这是主命令
// A command can have sub-commands, just like in command lines tools. Imagine `cargo help` and `cargo help run`.
#[command("upper")]
#[aliases("main")]
#[sub_commands(sub)]
#[description("main command")]
async fn upper_command(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    msg.reply(&ctx.http, "This is the main command!").await?;
    Ok(())
}

// 至输入`~upper sub`或`~upper secret`，则回复这是upper命令的子命令
// This will only be called if preceded by the `upper`-command.
#[command]
#[aliases("sub", "secret")]
#[description("This is `upper`'s sub-command.")]
async fn sub(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    msg.reply(&ctx.http, "This is a sub command!").await?;
    Ok(())
}

// ------------------------------math------------------------------
// 定义math命令，输入`~math * 5,7`（数值间隔可以是`,`或`, `），bot会输出结果`35`
#[group]
#[prefix = "math"] // Sets a single prefix for this group. So one has to call commands in this group via `~math` instead of just `~`.
#[commands(add, subtract, multiply, divide)]
struct Math;

#[command]
#[aliases("+")] // Lets us also call `~math +` instead of just `~math add`.
async fn add(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let first = args.single::<f64>()?;
    let second = args.single::<f64>()?;

    let res = first + second;

    msg.channel_id.say(&ctx.http, &res.to_string()).await?;

    Ok(())
}

#[command]
#[aliases("-")] // Lets us also call `~math -` instead of just `~math subtract`.
async fn subtract(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let first = args.single::<f64>()?;
    let second = args.single::<f64>()?;

    let res = first - second;

    msg.channel_id.say(&ctx.http, &res.to_string()).await?;

    Ok(())
}

#[command]
#[aliases("*")] // Lets us also call `~math *` instead of just `~math multiply`.
async fn multiply(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let first = args.single::<f64>()?;
    let second = args.single::<f64>()?;

    let res = first * second;

    msg.channel_id.say(&ctx.http, &res.to_string()).await?;

    Ok(())
}

#[command]
#[aliases("/")] // Lets us also call `~math /` instead of just `~math divide`.
async fn divide(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let first = args.single::<f64>()?;
    let second = args.single::<f64>()?;

    let res = first / second;

    msg.channel_id.say(&ctx.http, &res.to_string()).await?;

    Ok(())
}

// ------------------------------owner------------------------------
// 定义owner命令：
// 输入`~slow xxx`，需要指定一个参数，同一channel内两次调用间隔多少秒，必须在[0, 21600]范围内，即最大6小时，好像对管理员无效
// 输入`~role_id 用户名`，例如`~role_id srx-bot`，bot回复`Role-ID: @srx-bot`
#[group]
#[owners_only]
#[only_in(guilds)] // Limit all commands to be guild-restricted.
#[summary = "Commands for server owners"] // Summary only appears when listing multiple groups.
#[commands(slow, role_id)]
struct Owner;

// A function which acts as a "check", to determine whether to call a command.
//
// In this case, this command checks to ensure you are the owner of the message in order for the
// command to be executed. If the check fails, the command is not called.
#[check]
#[name = "Owner"]
async fn owner_check(_: &Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> Result<(), Reason> {
    if msg.author.id != 1218403146653503550 { // 这里设置为打印的id
        event!(parent: None, Level::WARN, "msg.author.id: {}", msg.author.id);
        return Err(Reason::User("Lacked owner permission".to_string()));
    }
    Ok(())
}

// 需要指定一个参数，同一channel内两次调用间隔多少秒，必须在[0, 21600]范围内，即最大6小时
// 可能对管理员无效，必须在群里发送设置，在私聊中设置会报错
#[command]
async fn slow(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let say_content = if let Ok(slow_mode_rate_seconds) = args.single::<u16>() {
        /*
        let name = msg.channel_id.name(&ctx.http).await?;
        let builder = EditChannel::new().name(name).rate_limit_per_user(slow_mode_rate_seconds); // 两次调用间隔多少秒
        */
        let builder = EditChannel::new().rate_limit_per_user(slow_mode_rate_seconds); // 两次调用间隔多少秒
        if let Err(e) = msg.channel_id.edit(&ctx.http, builder).await {
            event!(parent: None, Level::ERROR, "Error setting channel's slow mode rate: {:?}", e);
            format!("Failed to set slow mode to `{slow_mode_rate_seconds}` seconds.")
        } else {
            format!("Successfully set slow mode rate to `{slow_mode_rate_seconds}` seconds.")
        }
    } else if let Some(channel) = msg.channel_id.to_channel_cached(&ctx.cache) {
        let slow_mode_rate = channel.rate_limit_per_user.unwrap_or(0);
        format!("Current slow mode rate is `{slow_mode_rate}` seconds.")
    } else {
        "Failed to find channel in cache.".to_string()
    };

    msg.channel_id.say(&ctx.http, say_content).await?;

    Ok(())
}

// 输入`~role_id srx-bot`
#[command]
#[allowed_roles("srx", "srx-bot")] // 指定哪些用户可以调用该命令
async fn role_id(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let role_name = args.rest();
    let to_send = match msg.guild(&ctx.cache).as_deref().and_then(|g| g.role_by_name(role_name)) {
    /*
    let to_send = match msg.guild(&ctx.cache).as_deref().and_then(|g| {
        println!("roles: {:?}", g.roles); // 打印所有信息，这里g.roles是`HashMap<RoleId, Role>`
        g.role_by_name(role_name)
    }) {
    */
        Some(role_id) => format!("Role-ID: {role_id}"),
        None => format!("Could not find role name: {role_name:?}"),
    };

    if let Err(e) = msg.channel_id.say(&ctx.http, to_send).await {
        event!(parent: None, Level::ERROR, "Error sending message: {:?}", e);
    }

    Ok(())
}

// ------------------------------helper------------------------------
// 自定义帮助信息，当用户输入`~help`时要执行的代码
#[help]
#[individual_command_tip = "Hello!\n\nIf you want more information about a specific command, just pass the command as argument."] // 获取指定命令的信息
#[command_not_found_text = "Could not find: `{}`."] // 指定的命令不存在
#[max_levenshtein_distance(3)] // 计算指定命令和内置命令的名称相似度，模糊匹配
#[indention_prefix = "+"] // 指定子命令时显示的前缀符，默认`-`
#[lacking_permissions = "Hide"] // 隐藏当前用户没有权限的命令
#[lacking_role = "Nothing"] // If the user is nothing but lacking a certain role, we just display it.
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

// 执行命令前运行
#[hook]
async fn before(_ctx: &Context, msg: &Message, command_name: &str) -> bool {
    event!(parent: None, Level::INFO, "Got command '{}' by user '{}'", command_name, msg.author.name);
    true // 返回false则不会执行要调用的命令
}

// 执行命令后运行，打印成功或失败
#[hook]
async fn after(_ctx: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => event!(parent: None, Level::INFO, "Processed command '{}'", command_name),
        Err(e) => event!(parent: None, Level::ERROR, "Command '{}' returned error {:?}", command_name, e),
    }
}

// 如果用户发送的信息以`~`开头，但不是可调用的命令，则打印提示
#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
    event!(Level::WARN, "Could not find command named '{}'", unknown_command_name);
}

// 如果用户发送的信息以`~`开头，但不是可调用的命令，则打印提示
#[hook]
async fn normal_message(_ctx: &Context, msg: &Message) {
    event!(Level::WARN, "Message is not a command '{}'", msg.content);
}

// 如果是因为频率限制而报错，且是第一次遇到这个错误，则打印提示过几秒再试
#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError, _command_name: &str) {
    if let DispatchError::Ratelimited(info) = error {
        if info.is_first_try { // We notify them only once.
            let _ = msg
                .channel_id
                .say(&ctx.http, format!("Try this again in {} seconds.", info.as_secs()))
                .await;
        }
    }
}

// 当要执行的命令达到限制条件时，bot返回给用户的信息，比如提示等待
#[hook]
async fn delay_action(ctx: &Context, msg: &Message) {
    // You may want to handle a Discord rate limit if this fails.
    let _ = msg.react(ctx, '⏱').await;
}

pub async fn run_discord_bot(token: &str) {
    // Configure the client with your Discord bot token in the environment.
    // let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    // let token = "MTQ4MjM3MzIwNDM4MjE5MTg1MA.G6RyQv.LO9Y94eICOjnr0smwZFLZ9TdgmhfBquNminmyY".to_string();

    let http = Http::new(token);

    // We will fetch your bot's owners and id
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else if let Some(owner) = &info.owner {
                owners.insert(owner.id);
            }
            match http.get_current_user().await {
                Ok(bot_id) => (owners, bot_id.id),
                Err(e) => panic!("Could not access the bot id: {:?}", e),
            }
        },
        Err(e) => panic!("Could not access application info: {:?}", e),
    };

    let framework = StandardFramework::new()
        .before(before) // 在执行命令前运行，返回false则停止执行
        .after(after) // 在执行命令后运行
        .unrecognised_command(unknown_command) // 如果用户发送的信息以`~`开头，但不是可调用的命令，则打印提示
        .normal_message(normal_message) // 如果用户发送的信息以`~`开头，但不是可调用的命令，则打印提示
        .on_dispatch_error(dispatch_error) // 如果是因为频率限制而报错，且是第一次遇到这个错误，则打印提示过几秒再试
        .bucket("about",
            BucketBuilder::default().limit(2).time_span(10).delay(5) // 限制complicated命令每10秒只能用2次，且两次调用间隔必须大于5秒
                .limit_for(LimitedFor::Channel) // 将该限制应用在channel上
                .await_ratelimits(1) // 当调用命令超过限制条件时，可以有1个命令等待限制解除后再执行，如果设为0则表示不等待直接停止执行
                .delay_action(delay_action) // 当要执行的命令达到限制条件时，bot返回给用户的信息，比如提示等待
        ).await
        // `#[group]`宏为命令生成该组的静态实例，`#name_GROUP`是大写的命令名加上`_GROUP`后缀
        .help(&MY_HELP) // 当用户输入`~help`时要执行的代码
        .group(&GENERAL_GROUP)
        .group(&MATH_GROUP)
        .group(&OWNER_GROUP);

    framework.configure(
        Configuration::new().with_whitespace(true)
            .on_mention(Some(bot_id))
            .prefix("~") // 调用命令时要用的起始符
            .delimiters(vec![", ", ","]) // 参数分隔符
            .owners(owners) // 设置哪个用户是所有者，那些限制`#[owners_only]`的命令只有该用户可以调用
    );

    // dicord的开发者界面中，bot的`Presence Intent`和`Server Members Intent`需要开启
    let intents = GatewayIntents::all();
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    if let Err(e) = client.start().await {
        println!("Client error: {e:?}");
    }
}
