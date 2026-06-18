use std::io::{self, Read};

use crate::commands::bootstrap_admin::{self, BootstrapAdminOptions, EXIT_ALREADY_EXISTS};

pub enum Command {
    Serve,
    BootstrapAdmin(BootstrapAdminOptions),
    OpenApi,
}

pub fn parse() -> Command {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("serve") => Command::Serve,
        Some("bootstrap-admin") => Command::BootstrapAdmin(parse_bootstrap_admin(&mut args)),
        Some("openapi") | Some("--openapi") => Command::OpenApi,
        Some("help") | Some("--help") | Some("-h") => {
            print_help();
            std::process::exit(0);
        }
        Some(cmd) => {
            eprintln!("unknown command: {cmd}\n");
            print_help();
            std::process::exit(2);
        }
    }
}

fn parse_bootstrap_admin(args: &mut impl Iterator<Item = String>) -> BootstrapAdminOptions {
    let mut email: Option<String> = None;
    let mut password_stdin = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--email" => {
                email = args.next();
            }
            "--password-stdin" => {
                password_stdin = true;
            }
            "--help" | "-h" => {
                print_bootstrap_help();
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown bootstrap-admin argument: {other}\n");
                print_bootstrap_help();
                std::process::exit(2);
            }
        }
    }

    let email = email
        .or_else(|| std::env::var("AUTHSTACK_BOOTSTRAP_EMAIL").ok())
        .unwrap_or_default();

    let password = if password_stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .expect("failed to read password from stdin");
        buf.trim_end_matches(['\n', '\r']).to_string()
    } else {
        std::env::var("AUTHSTACK_BOOTSTRAP_PASSWORD").unwrap_or_default()
    };

    BootstrapAdminOptions { email, password }
}

pub async fn run_bootstrap_admin(options: BootstrapAdminOptions) -> ! {
    dotenvy::dotenv().ok();

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => {
            eprintln!("error: DATABASE_URL is required");
            std::process::exit(3);
        }
    };

    if options.email.is_empty() {
        eprintln!("error: email is required (--email or AUTHSTACK_BOOTSTRAP_EMAIL)");
        print_bootstrap_help();
        std::process::exit(2);
    }

    if options.password.is_empty() {
        eprintln!(
            "error: password is required (AUTHSTACK_BOOTSTRAP_PASSWORD or --password-stdin)"
        );
        print_bootstrap_help();
        std::process::exit(2);
    }

    match bootstrap_admin::run(&database_url, options).await {
        Ok(user) => {
            println!("created instance admin");
            println!("id:    {}", user.id);
            println!("email: {}", user.email);
            std::process::exit(0);
        }
        Err(err) => {
            let message = err.to_string();
            if message.contains("already exist") {
                eprintln!("error: {message}");
                std::process::exit(EXIT_ALREADY_EXISTS);
            }
            if message.contains("email is required")
                || message.contains("valid email")
                || message.contains("password must be")
            {
                eprintln!("error: {message}");
                std::process::exit(2);
            }
            eprintln!("error: {err:#}");
            std::process::exit(3);
        }
    }
}

fn print_help() {
    eprintln!(
        "\
authstack — multi-tenant authentication service

USAGE:
    authstack [COMMAND]

COMMANDS:
    serve             Start the HTTP API server (default)
    bootstrap-admin   Create the first instance admin (CLI only, empty database)
    openapi           Print the OpenAPI specification as JSON
    help              Show this help message

Run `authstack bootstrap-admin --help` for bootstrap options."
    );
}

fn print_bootstrap_help() {
    eprintln!(
        "\
Create the first instance admin. Refuses to run when any admin_user row exists.

USAGE:
    authstack bootstrap-admin [OPTIONS]

OPTIONS:
    --email <EMAIL>    Admin email (or set AUTHSTACK_BOOTSTRAP_EMAIL)
    --password-stdin   Read password from stdin (or set AUTHSTACK_BOOTSTRAP_PASSWORD)
    -h, --help         Show this help message

ENVIRONMENT:
    DATABASE_URL                 PostgreSQL connection string (required)
    AUTHSTACK_BOOTSTRAP_EMAIL    Admin email
    AUTHSTACK_BOOTSTRAP_PASSWORD Admin password

EXAMPLES:
    AUTHSTACK_BOOTSTRAP_EMAIL=admin@example.com \\
    AUTHSTACK_BOOTSTRAP_PASSWORD='secret' \\
      authstack bootstrap-admin

    echo 'secret' | authstack bootstrap-admin --email admin@example.com --password-stdin"
    );
}
