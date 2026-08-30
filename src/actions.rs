//! Validation of SoulsTAS action lines.
//!
//! Reference: https://github.com/Vinjul1704/SoulsTAS

pub const ACTION_NAMES: &[&str] = &[
    "nothing",
    "fps",
    "frame",
    "key",
    "key_alternative",
    "gamepad",
    "mouse",
    "await",
    "pause",
];

pub fn is_action(name: &str) -> bool {
    ACTION_NAMES.contains(&name)
}

fn is_num(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

fn expect_len(name: &str, args: &[String], n: usize) -> Result<(), String> {
    if args.len() != n {
        return Err(format!(
            "action '{}' expects {} argument(s), got {}",
            name,
            n,
            args.len()
        ));
    }
    Ok(())
}

fn expect_one_of(value: &str, allowed: &[&str], what: &str) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "invalid {what} '{value}', expected one of: {}",
            allowed.join(", ")
        ))
    }
}

fn expect_nums(args: &[String], range: std::ops::Range<usize>) -> Result<(), String> {
    for i in range {
        if !is_num(&args[i]) {
            return Err(format!("argument '{}' must be a number", args[i]));
        }
    }
    Ok(())
}

/// Validates a fully-evaluated action line. Returns a human readable error on failure.
pub fn validate(name: &str, args: &[String]) -> Result<(), String> {
    let up_down = ["down", "up"];

    match name {
        "nothing" => expect_len(name, args, 0),

        "fps" | "frame" => {
            expect_len(name, args, 1)?;
            expect_nums(args, 0..1)
        }

        "key" | "key_alternative" => {
            expect_len(name, args, 2)?;
            expect_one_of(&args[0], &up_down, "key state")?;
            if args[1].is_empty() {
                return Err("key name must not be empty".into());
            }
            Ok(())
        }

        "gamepad" => {
            if args.is_empty() {
                return Err("action 'gamepad' expects a sub-action (button/stick/axis)".into());
            }
            match args[0].as_str() {
                "button" => {
                    expect_len("gamepad button", args, 3)?;
                    expect_one_of(&args[1], &up_down, "button state")?;
                    if args[2].is_empty() {
                        return Err("gamepad button name must not be empty".into());
                    }
                    Ok(())
                }
                "stick" => {
                    expect_len("gamepad stick", args, 4)?;
                    expect_one_of(&args[1], &["left", "right"], "stick")?;
                    expect_nums(args, 2..4)?;
                    let amount: f64 = args[3].parse().unwrap();
                    if !(0.0..=1.0).contains(&amount) {
                        return Err(format!(
                            "gamepad stick amount must be between 0 and 1, got {amount}"
                        ));
                    }
                    Ok(())
                }
                "axis" => {
                    expect_len("gamepad axis", args, 3)?;
                    if args[1].is_empty() {
                        return Err("gamepad axis name must not be empty".into());
                    }
                    expect_nums(args, 2..3)
                }
                other => Err(format!(
                    "invalid gamepad sub-action '{other}', expected one of: button, stick, axis"
                )),
            }
        }

        "mouse" => {
            if args.is_empty() {
                return Err("action 'mouse' expects a sub-action (button/scroll/move)".into());
            }
            match args[0].as_str() {
                "button" => {
                    expect_len("mouse button", args, 3)?;
                    expect_one_of(&args[1], &up_down, "button state")?;
                    if args[2].is_empty() {
                        return Err("mouse button name must not be empty".into());
                    }
                    Ok(())
                }
                "scroll" => {
                    expect_len("mouse scroll", args, 3)?;
                    expect_one_of(&args[1], &up_down, "scroll direction")?;
                    expect_nums(args, 2..3)
                }
                "move" => {
                    expect_len("mouse move", args, 3)?;
                    expect_nums(args, 1..3)
                }
                other => Err(format!(
                    "invalid mouse sub-action '{other}', expected one of: button, scroll, move"
                )),
            }
        }

        "await" => {
            if args.is_empty() {
                return Err("action 'await' expects a condition".into());
            }
            match args[0].as_str() {
                "position" | "position_alternative" => {
                    expect_len(&format!("await {}", args[0]), args, 5)?;
                    expect_nums(args, 1..5)
                }
                other => {
                    expect_one_of(
                        other,
                        &[
                            "focus",
                            "ingame",
                            "no_ingame",
                            "cutscene",
                            "no_cutscene",
                            "mainmenu",
                            "no_mainmenu",
                        ],
                        "await condition",
                    )?;
                    expect_len(&format!("await {other}"), args, 1)
                }
            }
        }

        "pause" => {
            if args.is_empty() {
                return Err("action 'pause' expects a sub-action (ms/input)".into());
            }
            match args[0].as_str() {
                "ms" => {
                    expect_len("pause ms", args, 2)?;
                    expect_nums(args, 1..2)
                }
                "input" => expect_len("pause input", args, 1),
                other => Err(format!(
                    "invalid pause sub-action '{other}', expected one of: ms, input"
                )),
            }
        }

        other => Err(format!("unknown action '{other}'")),
    }
}
