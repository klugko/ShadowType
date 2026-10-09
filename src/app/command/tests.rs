use super::*;

#[test]
fn quits_like_vim() {
    for input in ["q", ":q", "q!", "quit", "wq", "  x  "] {
        assert_eq!(parse(input), Ok(Command::Quit), "{input}");
    }
}

#[test]
fn session_commands_take_optional_values() {
    assert_eq!(parse("words"), Ok(Command::Words(None)));
    assert_eq!(parse("words 25"), Ok(Command::Words(Some(25))));
    assert_eq!(parse("time 60"), Ok(Command::Time(Some(60))));
    assert_eq!(
        parse("code ts"),
        Ok(Command::Code(Some(CodeLanguage::TypeScript)))
    );
    assert!(matches!(
        parse("words lots"),
        Err(CommandError::InvalidArgument(_))
    ));
    assert!(matches!(
        parse("time 0"),
        Err(CommandError::InvalidArgument(_))
    ));
}

#[test]
fn lengths_out_of_range_are_refused_with_the_range() {
    assert_eq!(
        parse("words 9999").map_err(|error| error.to_string()),
        Err("E474: Invalid argument: 9999, use 1 to 500".to_owned())
    );
    assert!(parse("time 4").is_err());
    assert_eq!(parse("time 600"), Ok(Command::Time(Some(600))));
}

#[test]
fn language_accepts_natural_and_code_languages() {
    assert_eq!(
        parse("lang french"),
        Ok(Command::Language(Language::French))
    );
    assert_eq!(
        parse("lang rust"),
        Ok(Command::CodeLanguage(CodeLanguage::Rust))
    );
    assert_eq!(parse("lang"), Err(CommandError::MissingArgument("lang")));
}

#[test]
fn set_supports_toggles_and_assignments() {
    assert_eq!(
        parse("set punctuation"),
        Ok(Command::Set(Setting::Punctuation(true)))
    );
    assert_eq!(
        parse("set nonumbers"),
        Ok(Command::Set(Setting::Numbers(false)))
    );
    assert_eq!(
        parse("set theme=mono"),
        Ok(Command::Set(Setting::Theme(Theme::Mono)))
    );
    assert_eq!(
        parse("set server = ws://10.0.0.2:8080"),
        Ok(Command::Set(Setting::Server(
            "ws://10.0.0.2:8080".to_owned()
        )))
    );
    assert!(parse("set username=").is_err());
    assert!(parse("set colour=red").is_err());
}

#[test]
fn set_toggles_the_mascot_animations_mouse_and_chooses_the_look() {
    assert_eq!(
        parse("set nomascot"),
        Ok(Command::Set(Setting::Mascot(false)))
    );
    assert_eq!(
        parse("set animations"),
        Ok(Command::Set(Setting::Animations(true)))
    );
    assert_eq!(
        parse("set nomouse"),
        Ok(Command::Set(Setting::Mouse(false)))
    );
    assert_eq!(
        parse("set discreet"),
        Ok(Command::Set(Setting::Discreet(true)))
    );
    assert_eq!(
        parse("set look=commit"),
        Ok(Command::Set(Setting::Look(Look::Commit)))
    );
    assert!(parse("set look=poem").is_err());
    assert_eq!(
        parse("set theme=vscode"),
        Ok(Command::Set(Setting::Theme(Theme::VsCode)))
    );
    assert_eq!(
        parse("set icons=nerd"),
        Ok(Command::Set(Setting::Icons(Icons::Nerd)))
    );
    assert_eq!(
        parse("set notrail"),
        Ok(Command::Set(Setting::Trail(false)))
    );
}

#[test]
fn join_validates_the_room_code() {
    assert_eq!(
        parse("join fk72ad"),
        Ok(Command::Join("FK72AD".parse().expect("valid")))
    );
    assert!(matches!(
        parse("join FK72A0"),
        Err(CommandError::InvalidArgument(_))
    ));
}

#[test]
fn edit_keeps_the_path() {
    assert_eq!(
        parse("e src/main.rs"),
        Ok(Command::Edit(PathBuf::from("src/main.rs")))
    );
    assert_eq!(
        parse("e \"notes/my code.rs\""),
        Ok(Command::Edit(PathBuf::from("notes/my code.rs")))
    );
}

#[test]
fn file_arguments_start_from_the_home_directory_with_a_tilde() {
    let home = Path::new("/home/ada");
    let cases = [
        ("~", "/home/ada"),
        ("~/projects/lib.rs", "/home/ada/projects/lib.rs"),
        ("~ada/lib.rs", "~ada/lib.rs"),
        ("~notes.txt", "~notes.txt"),
        ("a/~/b.rs", "a/~/b.rs"),
        ("src/main.rs", "src/main.rs"),
    ];
    for (argument, path) in cases {
        assert_eq!(
            file_argument(argument, Some(home)),
            PathBuf::from(path),
            "{argument}"
        );
    }
    assert_eq!(file_argument("~/lib.rs", None), PathBuf::from("~/lib.rs"));
}

#[cfg(windows)]
#[test]
fn file_arguments_take_a_backslash_after_the_tilde_on_windows() {
    let home = Path::new(r"C:\Users\ada");
    assert_eq!(
        file_argument(r"~\code\main.rs", Some(home)),
        home.join(r"code\main.rs")
    );
}

#[test]
fn file_arguments_lose_the_quotes_around_them() {
    let cases = [
        (r#""C:\a b\c.rs""#, r"C:\a b\c.rs"),
        ("'x y'", "x y"),
        (r#"""#, r#"""#),
        (r#""x'"#, r#""x'"#),
        ("my notes.txt", "my notes.txt"),
        (r#""~/my notes.txt""#, "/home/ada/my notes.txt"),
    ];
    for (argument, path) in cases {
        assert_eq!(
            file_argument(argument, Some(Path::new("/home/ada"))),
            PathBuf::from(path),
            "{argument}"
        );
    }
}

#[test]
fn unknown_commands_use_vim_error_codes() {
    assert_eq!(
        parse("frobnicate").map_err(|error| error.to_string()),
        Err("E492: Not an editor command: frobnicate".to_owned())
    );
}

#[test]
fn completion_cycles_through_matches() {
    assert_eq!(complete("h", 0), Some("history"));
    assert_eq!(complete("h", 1), Some("help"));
    assert_eq!(complete("h", 2), Some("history"));
    assert_eq!(complete("zz", 0), None);
    assert_eq!(complete("join ab", 0), None);
}
