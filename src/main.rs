#[macro_use]
extern crate serenity;
#[macro_use]
extern crate diesel;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use log::*;
use serenity::{
    client::bridge::gateway::ShardManager,
    framework::standard::{
        help_commands, Args, CommandOptions, DispatchError, HelpBehaviour, StandardFramework,
    },
    model::{channel::Message, Permissions},
    prelude::*,
};
use std::{env, sync::Arc};

mod blackjack;
mod handler;
mod lockdown;
mod logger;
mod reaction_roles;
mod rules;
mod schema;
mod util;

mod interaction {
    pub mod wait;
}

mod models {
    pub mod bank;
    pub mod fav;
    pub mod reaction_role;
    pub mod tag;
}

mod commands {
    pub mod about;
    pub mod ban;
    pub mod account {
        pub mod blackjack;
        pub mod general;
        pub mod slot;
    }
    pub mod choose;
    pub mod fav;
    pub mod kick;
    pub mod lockdown;
    pub mod quote;
    pub mod reaction_roles;
    pub mod roll;
    pub mod rules;
    pub mod xkcd;
    pub mod twitch;
}

struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct DatabaseConnection;

impl TypeMapKey for DatabaseConnection {
    type Value = Arc<Mutex<PgConnection>>;
}

struct Waiter;

impl TypeMapKey for Waiter {
    type Value = Arc<Mutex<self::interaction::wait::Wait>>;
}

struct ReactionRolesState;

impl TypeMapKey for ReactionRolesState {
    type Value = Arc<Mutex<self::reaction_roles::State>>;
}

struct LockdownState;

impl TypeMapKey for LockdownState {
    type Value = Arc<Mutex<self::lockdown::State>>;
}

struct RulesState;

impl TypeMapKey for RulesState {
    type Value = Arc<Mutex<self::rules::State>>;
}

struct BlackjackState;

impl TypeMapKey for BlackjackState {
    type Value = Arc<Mutex<self::blackjack::State>>;
}

command!(setstatus(ctx, _msg, _args) {
    ctx.set_game(serenity::model::gateway::Game::listening("$help"));
});

fn main() {
    // load .env file
    kankyo::load().expect("no env file");
    // setup logging
    logger::setup_logger().expect("Could not setup logging");
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let mut client = Client::new(&token, handler::Handler).expect("Err creating client");

    let conn = Arc::new(Mutex::new(
        PgConnection::establish(
            &env::var("DATABASE_URL").expect("Expected a database in the environment"),
        )
        .expect("Error connecting to database"),
    ));

    let waiter = Arc::new(Mutex::new(self::interaction::wait::Wait::new()));

    let rr_state = Arc::new(Mutex::new(self::reaction_roles::State::load_set()));

    let rules_state = Arc::new(Mutex::new(self::rules::State::load()));

    let blackjack_state = Arc::new(Mutex::new(self::blackjack::State::load(conn.clone())));

    {
        let mut data = client.data.lock();
        data.insert::<DatabaseConnection>(conn);
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
        data.insert::<Waiter>(waiter);
        data.insert::<ReactionRolesState>(rr_state);
        data.insert::<RulesState>(rules_state);
        data.insert::<BlackjackState>(blackjack_state);
    }

    client.with_framework(
        StandardFramework::new()
            .configure(|c| {
                c.allow_whitespace(true)
                    .on_mention(true)
                    .prefix("$")
                    .prefix_only_cmd(commands::about::about)
                    .delimiter(" ")
            })
            .before(|_ctx, msg, command_name| {
                debug!(
                    "Got command '{}' by user '{}'",
                    command_name, msg.author.name
                );

                true
            })
            .after(|_, _, command_name, error| match error {
                Ok(()) => debug!("Processed command '{}'", command_name),
                Err(why) => debug!("Command '{}' returned error {:?}", command_name, why),
            })
            .unrecognised_command(|_, _, unknown_command_name| {
                debug!("Could not find command named '{}'", unknown_command_name);
            })
            .message_without_command(|_, message| {
                debug!("Message is not a command '{}'", message.content);
            })
            .on_dispatch_error(|_ctx, msg, error| {
                if let DispatchError::RateLimited(seconds) = error {
                    let _ = msg
                        .channel_id
                        .say(&format!("Versuche es in {} sekunden noch einmal.", seconds));
                }
            })
            .simple_bucket("slotmachine", 10)
            .simple_bucket("blackjack", 600)
            // commands
            .command("setstatus", |c| {
                c.desc("Setzt den Status des Bots")
                .num_args(0)
                .required_permissions(Permissions::MANAGE_ROLES | Permissions::MANAGE_CHANNELS)
                .cmd(setstatus)
            })
            .command("about", |c| {
                c.desc("Gibt eine kurze Info über den Bot")
                    .usage("about")
                    .num_args(0)
                    .cmd(commands::about::about)
            })
            .command("roll", |c| {
                c.desc("Rollt x Würfel mit y Augen.")
                    .num_args(2)
                    .example("1 6")
                    .usage(".roll x y")
                    .cmd(commands::roll::roll)
            })
            .command("choose", |c| {
                c.desc("Wählt eines der übergebenen Dinge.")
                    .example(r#"a "b mit spaces""#)
                    .usage(".choose apfel birne")
                    .cmd(commands::choose::choose)
            })
            .command("xkcd", |c| {
                c.desc("Postet einen Xkcd comic")
                .num_args(1)
                .example("1425")
                .cmd(commands::xkcd::xkcd)
            })
            // .command("kick", |c| {
            //     c.check(admin_check)
            //         .desc("Kickt alle mentioned user")
            //         .guild_only(true)
            //         .cmd(commands::kick::kick)
            // })
            // .command("ban", |c| {
            //     c.check(admin_check)
            //         .desc("Bannt alle mentioned user")
            //         .usage("ban x ...")
            //         .example("@user")
            //         .guild_only(true)
            //         .cmd(commands::ban::ban)
            // })
            .command("quote", |c|
                c.desc("Zitiert eine Nachricht")
                    .num_args(1)
                    .guild_only(true)
                    .usage("quote message_link")
                    .cmd(commands::quote::quote))
            .command("twitch", |c| {
                    c.desc("Macht Dinge mit Twitch Streams")
                    .num_args(1)
                    .example("1425")
                    .cmd(commands::twitch::twitch)
            })
            // .command("untagged", |c| {
            //     c.desc("Direkt an den Bot schreiben um untagged favs zu löschen/labeln. (Dazu dann auf 🗑 oder 🏷 klicken)")
            //         .usage("untagged")
            //         .num_args(0)
            //         .dm_only(true)
            //         .cmd(commands::fav::untagged)
            // })
            // .command("lockdown", |c| {
            //     c.desc("Nimmt allen Schreib & Reaction Rechte außer den mods.")
            //     .required_permissions(Permissions::MANAGE_ROLES | Permissions::MANAGE_CHANNELS)
            //     .num_args(0)
            //     .guild_only(true)
            //     .cmd(commands::lockdown::lockdown)
            // })
            // .command("unlock", |c| {
            //     c.desc("Setzt Schreib & Reaction Rechte wieder auf den ursprungszustand zurück.")
            //     .required_permissions(Permissions::MANAGE_ROLES | Permissions::MANAGE_CHANNELS)
            //     .num_args(0)
            //     .guild_only(true)
            //     .cmd(commands::lockdown::unlock)
            // })
            .group("Account", |g| {
                g.prefix("acc")
                .desc("Befehle im Zusammenhang mit deinem Konto")
                .default_cmd(commands::account::general::payday)
                .command("createaccount", |c| {
                    c.desc("Erstellt eine Bank für dich oder gibt dir deinen Kontostand")
                        .usage("createaccount")
                        .num_args(0)
                        .cmd(commands::account::general::createaccount)
                })
                .command("payday", |c| {
                    c.desc("Erhöht max alle 24std deinen Kontostand um 1000")
                        .known_as("paydaddy")
                        .usage("payday")
                        .num_args(0)
                        .cmd(commands::account::general::payday)
                })
                .command("leaderboard", |c| {
                    c.desc("Listet die Glücklichen und Süchtigen auf")
                        .usage("leaderboard")
                        .num_args(0)
                        .cmd(commands::account::general::leaderboard)
                })
                .command("transfer", |c| {
                    c.desc("Für den Sunshower-Moment. Beispiel: ")
                        .usage("transfer 1000 @HansTrashy")
                        .example("1000 @user1 @user2")
                        .cmd(commands::account::general::transfer)
                })
                .command("slot", |c| {
                    c.bucket("slotmachine")
                        .desc("Setzt x von deiner Bank, limitiert auf 1x alle 10 Sekunden")
                        .usage("slot x")
                        .example("1000")
                        .num_args(1)
                        .cmd(commands::account::slot::play)
                })
                .command("blackjack", |c| {
                    c.bucket("blackjack")
                        .desc("Spiele eine/mehrere runden Blackjack gegen die Bank")
                        .usage("blackjack x")
                        .example("1000")
                        .num_args(1)
                        .cmd(commands::account::blackjack::play)
                })
            })
            .group("Grünbuch", |g| {
                g.prefix("fav")
                .desc("Befehle für Grünbuch")
                .default_cmd(commands::fav::fav)
                .command("post", |c| {
                    c.desc("Postet einen zufälligen fav unter berücksichtigung der label.")
                    .example("taishi wichsen")
                    .cmd(commands::fav::fav)
                })
                .command("untagged", |c| {
                    c.desc("Direkt an den Bot schreiben um untagged favs zu löschen/labeln. (Dazu dann auf 🗑 oder 🏷 klicken)")
                    .usage("untagged")
                    .num_args(0)
                    .dm_only(true)
                    .cmd(commands::fav::untagged)
                })
                .command("add", |c| {
                    c.desc("Manuell einen fav per link hinzufügen")
                    .num_args(1)
                    .dm_only(true)
                    .cmd(commands::fav::add)
                })
            })
            .group("rules", |g| {
                g.prefix("rules")
                .desc("Befehle im Zusammenhang mit den Regeln.")
                .default_cmd(commands::rules::de)
                .command("de", |c| {
                    c.desc("Sendet dir die Regeln per DM.")
                    .num_args(0)
                    .cmd(commands::rules::de)
                })
                .command("en", |c| {
                    c.desc("Sendet dir die Regeln auf Englisch.")
                    .num_args(0)
                    .cmd(commands::rules::en)
                })
                .command("seten", |c| {
                    c.desc("Setzt die en Regeln")
                    .example("Regeltext")
                    .required_permissions(Permissions::MANAGE_ROLES)
                    .cmd(commands::rules::seten)
                })
                .command("setde", |c| {
                    c.desc("Setzt die de Regeln")
                    .example("Regeltext")
                    .required_permissions(Permissions::MANAGE_ROLES)
                    .cmd(commands::rules::setde)
                })
                .command("adden", |c| {
                    c.desc("Fügt Text and die en Regeln an")
                    .example("Regeltexterweiterung")
                    .required_permissions(Permissions::MANAGE_ROLES)
                    .cmd(commands::rules::adden)
                })
                .command("addde", |c| {
                    c.desc("Fügt Text and die de Regeln an")
                    .example("Regeltexterweiterung")
                    .required_permissions(Permissions::MANAGE_ROLES)
                    .cmd(commands::rules::addde)
                })
                .command("post", |c| {
                    c.desc("Lässt den bot die regeln posten")
                    .num_args(1)
                    .example("de")
                    .required_permissions(Permissions::MANAGE_ROLES)
                    .cmd(commands::rules::post)
                })
            })
            .group("Reaction Roles", |g| {
                g.prefix("rr")
                .required_permissions(Permissions::MANAGE_ROLES)
                .desc("Befehle für Reaction Roles Setup")
                .default_cmd(commands::reaction_roles::listrr)
                .command("create", |c| {
                    c.desc("Fügt eine neue Reaction Role zu einer gruppe hinzu.")
                    .example("🧀 gruppenname rollenname")
                    .cmd(commands::reaction_roles::createrr)
                })
                .command("remove", |c| { 
                    c.desc("Entfernt eine Reaction Role")
                    .example("🧀 rollenname")
                    .cmd(commands::reaction_roles::removerr)
                })
                .command("list", |c| {
                    c.desc("Auflistung aller ReactionRoles").usage("rr").cmd(commands::reaction_roles::listrr)
                })
                .command("postgroups", |c| {
                    c.desc("Postet die Reaction Nachrichten").cmd(commands::reaction_roles::postrrgroups)
                })
            })
            .customised_help(help_commands::with_embeds, |c| {
                c.individual_command_tip("Wenn du genaueres über einen Befehl wissen willst übergib ihn einfach als Argument.")
                .command_not_found_text("Konnte `{}` nicht finden.")
                .max_levenshtein_distance(3)
                .lacking_permissions(HelpBehaviour::Hide)
                .lacking_role(HelpBehaviour::Nothing)
                .wrong_channel(HelpBehaviour::Strike)
                .suggestion_text("Meintest du vielleicht `{}`?")
                .no_help_available_text("Dafür gibt es leider noch keine Hilfe.")
                .striked_commands_tip_in_guild(Some("Durchgestrichene Befehle können nur auf einem Server mit dem Bot benutzt werden.".to_string()))
                .striked_commands_tip_in_direct_message(Some("Durchgestrichene Befehle können nur in Direktnachrichten mit dem Bot benutzt werden.".to_string()))
            }),
    );

    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}

fn admin_check(_: &mut Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> bool {
    if let Some(member) = msg.member() {
        if let Ok(permissions) = member.permissions() {
            return permissions.administrator();
        }
    }
    false
}

#[cfg(test)]
mod tests {}
