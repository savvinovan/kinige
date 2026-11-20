//! `keychain-check write|read|delete`: the three calls the app's secret store makes (ADR-0021), on a throwaway service name and a fixed fake
//! secret. Prints the build tag and one line of result; the exit status is 0 for success, 1 for a keychain error, 2 for usage.

#[cfg(target_os = "macos")]
fn main() {
    use security_framework::passwords::{
        PasswordOptions, delete_generic_password_options, generic_password,
        set_generic_password_options,
    };
    const SERVICE: &str = "me.savvinov.kinige.keychain-check";
    const ACCOUNT: &str = "telegram-bot-token";
    const SECRET: &[u8] = b"0:fake-secret-for-the-keychain-check";
    let tag = option_env!("BUILD_TAG").unwrap_or("none");
    let options = || {
        let mut options = PasswordOptions::new_generic_password(SERVICE, ACCOUNT);
        options.set_access_synchronized(Some(false));
        options
    };
    let command = std::env::args().nth(1).unwrap_or_default();
    let outcome = match command.as_str() {
        "write" => set_generic_password_options(SECRET, options()).map(|()| "written".to_string()),
        "read" => generic_password(options()).map(|bytes| {
            if bytes == SECRET {
                "read the item".to_string()
            } else {
                "read something else".to_string()
            }
        }),
        "delete" => delete_generic_password_options(options()).map(|()| "deleted".to_string()),
        _ => {
            println!("usage: keychain-check write|read|delete");
            std::process::exit(2);
        }
    };
    match outcome {
        Ok(text) => println!("build {tag}: {command}: {text}"),
        Err(error) => {
            println!("build {tag}: {command}: error {} ({})", error.code(), error);
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the keychain check runs on macOS only");
    std::process::exit(2);
}
