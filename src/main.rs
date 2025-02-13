use game::Game;

mod character;
mod datafile;
mod event;
mod game;
mod item;
mod location;
mod log;
mod quest;
mod randomizer;

use crate::location::Location;
use clap::{crate_version, AppSettings, Clap};

/// Your filesystem as a dungeon!
#[derive(Clap)]
#[clap(global_setting = AppSettings::ColoredHelp)]
#[clap(version = crate_version!(), author = "Facundo Olano <facundo.olano@gmail.com>")]
struct Opts {
    #[clap(subcommand)]
    cmd: Option<Command>,

    /// Print succinct output when possible.
    #[clap(long, short, global = true)]
    quiet: bool,

    /// Print machine-readable output when possible.
    #[clap(long, global = true)]
    plain: bool,
}

#[derive(Clap)]
enum Command {
    /// Display the hero's status [default]
    #[clap(aliases=&["s", "status"], display_order=0)]
    Stat,

    /// Moves the hero to the supplied destination, potentially initiating battles along the way.
    #[clap(name = "cd", display_order = 1)]
    ChangeDir {
        /// Directory to move to.
        #[clap(default_value = "~")]
        destination: String,

        /// Attempt to avoid battles by running away.
        #[clap(long)]
        run: bool,

        /// Attempt to avoid battles by bribing the enemy.
        #[clap(long)]
        bribe: bool,

        /// Move the hero's to a different location without spawning enemies.
        /// Intended for scripts and shell integration.
        #[clap(short, long)]
        force: bool,
    },

    /// Inspect the directory contents, possibly finding treasure chests and hero tombstones.
    #[clap(name = "ls", display_order = 1)]
    Inspect,

    /// Buys an item from the shop.
    /// If name is omitted lists the items available for sale.
    #[clap(alias = "b", display_order = 2)]
    Buy { item: Option<String> },

    /// Uses an item from the inventory.
    #[clap(alias = "u", display_order = 3)]
    Use { item: Option<String> },

    /// Prints the quest todo list.
    #[clap(alias = "t", display_order = 4)]
    Todo,

    /// Resets the current game.
    Reset {
        /// Reset data files, losing cross-hero progress.
        #[clap(long)]
        hard: bool,
    },

    /// Prints the hero's current location
    #[clap(name = "pwd")]
    PrintWorkDir,

    /// Potentially initiates a battle in the hero's current location.
    Battle {
        /// Attempt to avoid battles by running away.
        #[clap(long)]
        run: bool,

        /// Attempt to avoid battles by bribing the enemy.
        #[clap(long)]
        bribe: bool,
    },
}

fn main() {
    let mut exit_code = 0;

    let opts: Opts = Opts::parse();
    log::init(opts.quiet, opts.plain);

    // reset --hard is a special case, it needs to work when we
    // fail to deserialize the game data -- e.g. on backward
    // incompatible changes
    if let Some(Command::Reset { hard: true }) = opts.cmd {
        datafile::remove();
    }

    let mut game = datafile::load().unwrap_or_else(|_| Game::new());

    match opts.cmd.unwrap_or(Command::Stat) {
        Command::Stat => log::status(&game),
        Command::ChangeDir {
            destination,
            run,
            bribe,
            force,
        } => {
            exit_code = change_dir(&mut game, &destination, run, bribe, force);
        }
        Command::Inspect => {
            game.inspect();
        }
        Command::Battle { run, bribe } => {
            exit_code = battle(&mut game, run, bribe);
        }
        Command::PrintWorkDir => println!("{}", game.location.path_string()),
        Command::Reset { .. } => game.reset(),
        Command::Buy { item } => shop(&mut game, &item),
        Command::Use { item } => use_item(&mut game, &item),
        Command::Todo => {
            let (todo, done) = game.quests.list(&game);
            log::quest_list(&todo, &done);
        }
    }

    datafile::save(&game).unwrap();
    std::process::exit(exit_code);
}

/// Attempt to move the hero to the supplied location, possibly engaging
/// in combat along the way.
fn change_dir(game: &mut Game, dest: &str, run: bool, bribe: bool, force: bool) -> i32 {
    if let Ok(dest) = Location::from(&dest) {
        if force {
            game.location = dest;
        } else if let Err(character::Dead) = game.go_to(&dest, run, bribe) {
            game.reset();
            return 1;
        }
    } else {
        println!("No such file or directory");
        return 1;
    }
    0
}

/// Potentially run a battle at the current location, independently from
/// the hero's movement.
fn battle(game: &mut Game, run: bool, bribe: bool) -> i32 {
    let mut exit_code = 0;
    if let Some(mut enemy) = game.maybe_spawn_enemy() {
        if let Err(character::Dead) = game.maybe_battle(&mut enemy, run, bribe) {
            game.reset();
            exit_code = 1;
        }
    }
    exit_code
}

/// Buy an item from the shop or list the available items if no item name is provided.
/// Shopping is only allowed when the player is at the home directory.
fn shop(game: &mut Game, item_name: &Option<String>) {
    if game.location.is_home() {
        if let Some(item_name) = item_name {
            let item_name = sanitize(item_name);
            match item::shop::buy(game, &item_name) {
                Err(item::shop::Error::NotEnoughGold) => {
                    println!("Not enough gold.")
                }
                Err(item::shop::Error::ItemNotAvailable) => {
                    println!("Item not available.")
                }
                Ok(()) => {}
            }
        } else {
            item::shop::list(game);
        }
    } else {
        println!("Shop is only allowed at home.")
    }
}

/// Use an item from the inventory or list the inventory contents if no item name is provided.
fn use_item(game: &mut Game, item_name: &Option<String>) {
    if let Some(item_name) = item_name {
        let item_name = sanitize(item_name);
        if let Err(game::ItemNotFound) = game.use_item(&item_name) {
            println!("Item not found.");
        }
    } else {
        println!("{}", log::format_inventory(&game));
    }
}

/// Return a clean version of an item/equipment name, including aliases
fn sanitize(name: &str) -> String {
    let name = name.to_lowercase();
    let name = match name.as_str() {
        "p" | "potion" => "potion",
        "e" | "escape" => "escape",
        "sw" | "sword" => "sword",
        "sh" | "shield" => "shield",
        n => n,
    };
    name.to_string()
}
