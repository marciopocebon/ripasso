/*  Ripasso - a simple password manager
    Copyright (C) 2019 Joakim Lundborg, Alexander Kjäll

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

extern crate cursive;
extern crate env_logger;
extern crate ripasso;

use self::cursive::traits::*;
use self::cursive::views::{
    Dialog, EditView, LinearLayout, OnEventView, SelectView, TextArea, TextView, CircularFocus,
};

use cursive::Cursive;
use cursive::menu::MenuTree;

use self::cursive::direction::Orientation;
use self::cursive::event::{Event, Key};

extern crate clipboard;
use self::clipboard::{ClipboardContext, ClipboardProvider};

use ripasso::pass;
use ripasso::pass::SignatureStatus;
use ripasso::pass::GitRepo;

use std::process;
use std::{thread, time};
use std::sync::{Arc, Mutex};

use unic_langid::LanguageIdentifier;

mod helpers;
mod wizard;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref CATALOG: gettext::Catalog = get_translation_catalog();
}

fn down(ui: &mut Cursive) -> () {
    ui.call_on_id("results", |l: &mut SelectView<pass::PasswordEntry>| {
        l.select_down(1);
    });
}

fn up(ui: &mut Cursive) -> () {
    ui.call_on_id("results", |l: &mut SelectView<pass::PasswordEntry>| {
        l.select_up(1);
    });
}

fn page_down(ui: &mut Cursive) -> () {
    let mut l = ui.find_id::<SelectView<pass::PasswordEntry>>("results").unwrap();
    l.select_down(ui.screen_size().y);
}

fn page_up(ui: &mut Cursive) -> () {
    let mut l = ui.find_id::<SelectView<pass::PasswordEntry>>("results").unwrap();
    l.select_up(ui.screen_size().y);
}

fn copy(ui: &mut Cursive) -> () {
    let l = ui.find_id::<SelectView<pass::PasswordEntry>>("results").unwrap();

    let sel = l.selection();

    if sel.is_none() {
        return;
    }

    let password = sel.unwrap().password();

    if password.is_err() {
        helpers::errorbox(ui, &password.unwrap_err());
        return;
    }

    let ctx_res = clipboard::ClipboardContext::new();
    if ctx_res.is_err() {
        helpers::errorbox(ui, &pass::Error::GenericDyn(format!("{}", &ctx_res.err().unwrap())));
        return;
    }
    let mut ctx: ClipboardContext = ctx_res.unwrap();
    ctx.set_contents(password.unwrap().to_owned()).unwrap();

    thread::spawn(|| {
        thread::sleep(time::Duration::from_secs(40));
        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        ctx.set_contents("".to_string()).unwrap();
    });

    ui.call_on_id("status_bar", |l: &mut TextView| {
        l.set_content(CATALOG.gettext("Copied password to copy buffer for 40 seconds"));
    });
}

fn do_delete(ui: &mut Cursive, repo_opt: GitRepo) -> () {
    ui.call_on_id("results", |l: &mut SelectView<pass::PasswordEntry>| {
        let sel = l.selection();

        if sel.is_none() {
            return;
        }

        let sel = sel.unwrap();

        let r = sel.delete_file(repo_opt);

        if r.is_err() {
            return;
        }

        let delete_id = l.selected_id().unwrap();
        l.remove_item(delete_id);
    });

    ui.pop_layer();
}

fn delete(ui: &mut Cursive, repo_opt: GitRepo) -> () {
    ui.add_layer(CircularFocus::wrap_tab(
    Dialog::around(TextView::new(CATALOG.gettext("Are you sure you want to delete the password?")))
        .button(CATALOG.gettext("Yes"), move |ui: &mut Cursive| {
            do_delete(ui, repo_opt.clone());
            ui.call_on_id("status_bar", |l: &mut TextView| {
                l.set_content(CATALOG.gettext("Password deleted"));
            });
        })
        .dismiss_button(CATALOG.gettext("Cancel"))));
}

fn open(ui: &mut Cursive, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) -> () {
    let password_entry_option: Option<Option<std::rc::Rc<ripasso::pass::PasswordEntry>>> = ui
        .call_on_id("results", |l: &mut SelectView<pass::PasswordEntry>| {
            l.selection()
        });

    let password_entry: pass::PasswordEntry = (*(match password_entry_option {
        Some(level_1) => {
            match level_1 {
                Some(level_2) => level_2,
                None => return
            }
        },
        None => return
    })).clone();

    let password = match password_entry.secret() {
        Ok(p) => p,
        Err(_e) => return
    };
    let d =
        Dialog::around(TextArea::new().content(password).with_id("editbox"))
            .button(CATALOG.gettext("Save"), move |s| {
                let new_password = s
                    .call_on_id("editbox", |e: &mut TextArea| {
                        e.get_content().to_string()
                    }).unwrap();
                let r = password_entry.update(new_password, repo_opt.clone(), password_store_dir.clone());
                if r.is_err() {
                    helpers::errorbox(s, &r.unwrap_err())
                }
            })
            .button(CATALOG.gettext("Generate"), move |s| {
                let new_password = ripasso::words::generate_password(6);
                s.call_on_id("editbox", |e: &mut TextArea| {
                    e.set_content(new_password);
                });
            })
            .dismiss_button(CATALOG.gettext("Ok"));

    let ev = OnEventView::new(d)
        .on_event(Key::Esc, |s| {
            s.pop_layer();
        });

    ui.add_layer(ev);
}

fn get_value_from_input(s: &mut Cursive, input_name: &str) -> Option<std::rc::Rc<String>> {
    let mut password= None;
    s.call_on_id(input_name, |e: &mut EditView| {
        password = Some(e.get_content());
    });
    return password;
}

fn create_save(s: &mut Cursive, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) -> () {
    let password = get_value_from_input(s, "new_password_input");
    if password.is_none() {
        return;
    }
    let password = password.unwrap();
    if *password == "" {
        return;
    }

    let path = get_value_from_input(s, "new_path_input");
    if path.is_none() {
        return;
    }
    let path = path.unwrap();
    if *path == "" {
        return;
    }

    let res = pass::new_password_file(path.clone(), password, repo_opt.clone(), password_store_dir.clone());

    let col = s.screen_size().x;
    if res.is_err() {
        helpers::errorbox(s, &res.err().unwrap())
    } else {
        s.call_on_id("results", |l: &mut SelectView<pass::PasswordEntry>| {
            let mut path_buf: std::path::PathBuf = pass::password_dir(password_store_dir.clone()).unwrap();

            let path_deref = (*path).clone();
            let path_iter = &mut path_deref.split("/").peekable();

            while let Some(p) = path_iter.next() {
                if path_iter.peek().is_some() {
                    path_buf.push(p);
                } else {
                    path_buf.push(format!("{}.gpg", p));
                }
            }

            match pass::PasswordEntry::load_from_git(&pass::password_dir(password_store_dir).unwrap(), &path_buf, repo_opt.clone()) {
                Ok(e) => l.add_item(create_label(&e, col), e),
                Err(_) => eprintln!("error")
            }
        });

        s.pop_layer();

        s.call_on_id("status_bar", |l: &mut TextView| {
            l.set_content(CATALOG.gettext("Created new password"));
        });
    }
}

fn create(ui: &mut Cursive, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) -> () {
    let mut fields = LinearLayout::vertical();
    let mut path_fields = LinearLayout::horizontal();
    let mut password_fields = LinearLayout::horizontal();
    path_fields.add_child(TextView::new(CATALOG.gettext("Path: "))
        .with_id("path_name")
        .fixed_size((10, 1)));
    path_fields.add_child(EditView::new()
            .with_id("new_path_input")
            .fixed_size((50, 1)));
    password_fields.add_child(TextView::new(CATALOG.gettext("Password: "))
        .with_id("password_name")
        .fixed_size((10, 1)));
    password_fields.add_child(EditView::new()
        .secret()
        .with_id("new_password_input")
        .fixed_size((50, 1)));
    fields.add_child(path_fields);
    fields.add_child(password_fields);

    let repo_opt2 = repo_opt.clone();
    let password_store_dir2 = password_store_dir.clone();

    let d =
        Dialog::around(fields)
            .title(CATALOG.gettext("Add new password"))
            .button(CATALOG.gettext("Generate"), move |s| {
                let new_password = ripasso::words::generate_password(6);
                s.call_on_id("new_password_input", |e: &mut EditView| {
                    e.set_content(new_password);
                });
            })
            .button(CATALOG.gettext("Save"), move |ui: &mut Cursive| {
                create_save(ui, repo_opt.clone(), password_store_dir.clone())
            })
            .dismiss_button(CATALOG.gettext("Cancel"));

    let ev = OnEventView::new(d)
        .on_event(Key::Esc, |s| {
            s.pop_layer();
        })
        .on_event(Key::Enter, move |ui: &mut Cursive| {
            create_save(ui, repo_opt2.clone(), password_store_dir2.clone())
        });

    ui.add_layer(ev);
}

fn delete_recipient(ui: &mut Cursive, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) -> () {
    let mut l = ui.find_id::<SelectView<pass::Recipient>>("recipients").unwrap();
    let sel = l.selection();

    if sel.is_none() {
        return;
    }

    let r = ripasso::pass::Recipient::remove_recipient_from_file(&sel.unwrap(), repo_opt, password_store_dir);

    if r.is_err() {
        helpers::errorbox(ui, &r.unwrap_err());
    } else {
        let delete_id = l.selected_id().unwrap();
        l.remove_item(delete_id);

        ui.call_on_id("status_bar", |l: &mut TextView| {
            l.set_content(CATALOG.gettext("Deleted team member from password store"));
        });
    }
}

fn delete_recipient_verification(ui: &mut Cursive, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) -> () {
    ui.add_layer(CircularFocus::wrap_tab(
        Dialog::around(TextView::new(CATALOG.gettext("Are you sure you want to remove this person?")))
            .button(CATALOG.gettext("Yes"), move |ui: &mut Cursive| {
                delete_recipient(ui, repo_opt.clone(), password_store_dir.clone())
            })
            .dismiss_button(CATALOG.gettext("Cancel"))));
}

fn add_recipient(ui: &mut Cursive, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) -> () {
    let l = &*get_value_from_input(ui, "key_id_input").unwrap();

    let recipient_result = pass::Recipient::new(l.clone());

    if recipient_result.is_err() {
        helpers::errorbox(ui, &recipient_result.err().unwrap());
    } else {
        let res = pass::Recipient::add_recipient_to_file(&recipient_result.unwrap(), repo_opt, password_store_dir.clone());
        if res.is_err() {
            helpers::errorbox(ui, &res.unwrap_err());
        } else {
            ui.pop_layer();
            ui.call_on_id("status_bar", |l: &mut TextView| {
                l.set_content(CATALOG.gettext("Added team member to password store"));
            });
        }
    }
}

fn add_recipient_dialog(ui: &mut Cursive, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) -> () {
    let mut recipient_fields = LinearLayout::horizontal();

    recipient_fields.add_child(TextView::new(CATALOG.gettext("GPG Key ID: "))
        .with_id("key_id")
        .fixed_size((16, 1)));

    let repo_opt2 = repo_opt.clone();
    let password_store_dir2 = password_store_dir.clone();

    let gpg_key_edit_view = OnEventView::new(EditView::new()
        .with_id("key_id_input")
        .fixed_size((50, 1)))
        .on_event(Key::Enter, move |ui: &mut Cursive| {
            add_recipient(ui, repo_opt.clone(), password_store_dir.clone())
        });

    recipient_fields.add_child(gpg_key_edit_view);

    let cf = CircularFocus::wrap_tab(
        Dialog::around(recipient_fields)
            .button(CATALOG.gettext("Yes"), move |ui: &mut Cursive| {
                add_recipient(ui, repo_opt2.clone(), password_store_dir2.clone())
            })
            .dismiss_button(CATALOG.gettext("Cancel")));

    let ev = OnEventView::new(cf)
        .on_event(Key::Esc, |s| {
            s.pop_layer();
        });

    ui.add_layer(ev);
}

fn view_recipients(ui: &mut Cursive, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) -> () {
    let recipients_res : Result<Vec<ripasso::pass::Recipient>, pass::Error> = ripasso::pass::Recipient::all_recipients(password_store_dir.clone());

    if recipients_res.is_err() {
        helpers::errorbox(ui, &recipients_res.err().unwrap());
        return ();
    }
    let recipients = recipients_res.unwrap();

    let mut recipients_view = SelectView::<pass::Recipient>::new()
        .h_align(cursive::align::HAlign::Left)
        .with_id("recipients");

    for recipient in recipients {
        recipients_view.get_mut().add_item(format!("{} {}", recipient.key_id.clone(), recipient.name.clone()), recipient);
    }

    let d = Dialog::around(recipients_view)
        .title(CATALOG.gettext("Team Members"))
        .dismiss_button("Ok");

    let ll = LinearLayout::new(Orientation::Vertical)
        .child(d)
        .child(LinearLayout::new(Orientation::Horizontal)
            .child(TextView::new(CATALOG.gettext("ins: Add | ")))
            .child(TextView::new(CATALOG.gettext("del: Remove"))));

    let repo_opt2 = repo_opt.clone();
    let password_store_dir2 = password_store_dir.clone();

    let recipients_event = OnEventView::new(ll)
        .on_event(Key::Del, move |ui: &mut Cursive| {
            delete_recipient_verification(ui, repo_opt.clone(), password_store_dir.clone())
        })
        .on_event(Key::Ins, move |ui: &mut Cursive| {
            add_recipient_dialog(ui, repo_opt2.clone(), password_store_dir2.clone())
        })
        .on_event(Key::Esc, |s| {
            s.pop_layer();
        });

    ui.add_layer(recipients_event);
}

fn substr(str: &String, start: usize, len: usize) -> String {
    return str.chars().skip(start).take(len).collect();
}

fn create_label(p: &pass::PasswordEntry, col: usize) -> String {
    let committed_by = p.committed_by.clone();
    let updated = p.updated;
    let name = substr(&match committed_by {
        Some(d) => d,
        None => CATALOG.gettext("n/a").to_string(),
    }, 0, 15);
    let mut verification_status = " ";
    if p.signature_status.is_some() {
        verification_status = match p.signature_status.as_ref().unwrap() {
            SignatureStatus::GoodSignature => "🔒",
            SignatureStatus::AlmostGoodSignature => "🔓",
            SignatureStatus::BadSignature => "⛔",
        }
    }
    return format!("{:4$} {} {} {}",
                p.name,
                verification_status,
                name,
                match updated {
                    Some(d) => format!("{}", d.format("%Y-%m-%d")),
                    None => CATALOG.gettext("n/a").to_string(),
                },
                _ = col - 12 - 15 - 9, // Optimized for 80 cols
            );
}

fn search(passwords: &pass::PasswordList, ui: &mut Cursive, query: &str) -> () {
    let col = ui.screen_size().x;
    let mut l = ui.find_id::<SelectView<pass::PasswordEntry>>("results").unwrap();

    let r_res = pass::search(&passwords, &String::from(query));
    if r_res.is_err() {
        helpers::errorbox(ui, &r_res.err().unwrap());
        return ();
    }
    let r = r_res.unwrap();
    l.clear();
    for p in &r {
        l.add_item(create_label(&p, col), p.clone());
    }
}

fn help() {
    println!("{}", CATALOG.gettext("A password manager that uses the file format of the standard unix password manager 'pass', implemented in Rust. Ripasso reads $HOME/.password-store/ by default, override this by setting the PASSWORD_STORE_DIR environmental variable."));
}

fn git_push(ui: &mut Cursive, repo_opt: GitRepo) {
    let res = pass::push(repo_opt);

    if res.is_err() {
        helpers::errorbox(ui, &res.unwrap_err());
    } else {
        ui.call_on_id("status_bar", |l: &mut TextView| {
            l.set_content(CATALOG.gettext("Pushed to remote git repository"));
        });
    }
}

fn git_pull(ui: &mut Cursive, passwords: pass::PasswordList, repo_opt: GitRepo, password_store_dir: Arc<Option<String>>) {
    let pull_res = pass::pull(repo_opt.clone());

    if pull_res.is_err() {
        helpers::errorbox(ui, &pull_res.unwrap_err());
    }

    let res = pass::populate_password_list(&passwords, repo_opt, password_store_dir);
    if res.is_err() {
        helpers::errorbox(ui, &res.unwrap_err());
    }

    let col = ui.screen_size().x;

    ui.call_on_id("results", |l: &mut SelectView<pass::PasswordEntry>| {
        l.clear();
        for p in passwords.lock().unwrap().iter() {
            l.add_item(create_label(&p, col), p.clone());
        }
    });
    ui.call_on_id("status_bar", |l: &mut TextView| {
        l.set_content(CATALOG.gettext("Pulled from remote git repository"));
    });
}

fn do_delete_last_word(ui: &mut Cursive, passwords: pass::PasswordList) -> () {
    ui.call_on_id("searchbox", |e: &mut EditView| {
        let s = e.get_content();
        let last_space = s.trim().rfind(" ");
        match last_space {
            Some(pos) => {
                e.set_content(s[0..pos+1].to_string());
            },
            None => {
                e.set_content("");
                ()
            }
        };
    });
    let search_text = ui.find_id::<EditView>("searchbox").unwrap().get_content();
    search(&passwords, ui, &search_text);
}

fn get_translation_catalog() -> gettext::Catalog {
    let locale = locale_config::Locale::current();

    let mut translation_locations = vec!["/usr/share/ripasso"];
    let translation_input_path = option_env!("TRANSLATION_INPUT_PATH");
    if translation_input_path.is_some() {
        translation_locations.insert(0, translation_input_path.unwrap());
    }
    if cfg!(debug_assertions) {
        translation_locations.insert(0, "./cursive/res");
    }

    for preferred in locale.tags_for("messages") {
        for loc in &translation_locations {
            let langid_res: Result<LanguageIdentifier, _> = format!("{}", preferred).parse();

            if langid_res.is_ok() {
                let file = std::fs::File::open(format!("{}/{}.mo", loc, langid_res.unwrap().get_language()));
                if file.is_ok() {
                    let catalog_res = gettext::Catalog::parse(file.unwrap());

                    if catalog_res.is_ok() {
                        return catalog_res.unwrap();
                    }
                }
            }
        }
    }

    return gettext::Catalog::empty();
}

fn main() {
    env_logger::init();

    let password_store_dir = Arc::new(match std::env::var("PASSWORD_STORE_DIR") {
        Ok(p) => Some(p),
        Err(_) => None
    });
    let args: Vec<String> = std::env::args().collect();

    match args.len() {
        1 => {}
        2 => {
            if args[1] == "-h" || args[1] == "--help" {
                help();
                std::process::exit(0);
            } else {
                eprintln!("{}", CATALOG.gettext("Unknown argument, usage: ripasso-cursive [-h|--help]"));
                process::exit(1);
            }
        },
        _ => {
            eprintln!("{}", CATALOG.gettext("Unknown argument, usage: ripasso-cursive [-h|--help]"));
            process::exit(1);
        }
    }

    if pass::password_dir(password_store_dir.clone()).is_err() {
        wizard::show_init_menu(password_store_dir.clone());
    }

    if pass::password_dir(password_store_dir.clone()).is_ok() {
        let mut gpg_id_file = pass::password_dir(password_store_dir.clone()).unwrap();
        gpg_id_file.push(".gpg-id");
        if !gpg_id_file.exists() {
            eprintln!("{}", CATALOG.gettext("You have pointed ripasso towards an existing directory without an .gpg-id file, this doesn't seem like a password store directory, quiting."));
            process::exit(1);
        }
    }
    let pdir_res = pass::password_dir(password_store_dir.clone());
    if pdir_res.is_err() {
        eprintln!("Error {:?}", pdir_res.err().unwrap());
        process::exit(1);
    }
    let repo_res = git2::Repository::open(pdir_res.unwrap());
    let mut repo_opt: GitRepo = Arc::new(None::<Mutex<git2::Repository>>);
    if repo_res.is_ok() {
        repo_opt = Arc::new(Some(Mutex::new(repo_res.unwrap())));
    }

    // verify that the git config is correct
    if repo_opt.is_some() {
        let config = git2::Config::open_default().unwrap();

        let user_name = config.get_string("user.name");

        if user_name.is_err() {
            eprintln!("{}", CATALOG.gettext("You haven't configured you name and email in git, doing so will make cooperation with your team easier, you can do it like this:\ngit config --global user.name \"John Doe\"\ngit config --global user.email \"email@example.com\"\n\nAlso consider configuring git to sign your commits with GPG:\ngit config --global user.signingkey 3AA5C34371567BD2\ngit config --global commit.gpgsign true"));
            process::exit(1);
        }
    }

    // Load and watch all the passwords in the background
    let (password_rx, passwords) = match pass::watch(repo_opt.clone(), password_store_dir.clone()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error {:?}", e);
            process::exit(1);
        }
    };

    let mut ui = Cursive::default();

    // Update UI on password change event
    let e = ui.cb_sink().send(Box::new(move |s: &mut Cursive| {
        let event = password_rx.try_recv();
        if let Ok(e) = event {
            if let pass::PasswordEvent::Error(ref err) = e {
                helpers::errorbox(s, err)
            }
        }
    }));

    if e.is_err() {
        eprintln!("Application error: {}", e.err().unwrap());
        return;
    }

    let (repo_opt2, password_store_dir2) = (repo_opt.clone(), password_store_dir.clone());
    let (repo_opt3, password_store_dir3) = (repo_opt.clone(), password_store_dir.clone());
    let (repo_opt4, password_store_dir4) = (repo_opt.clone(), password_store_dir.clone());
    let (repo_opt5, password_store_dir5) = (repo_opt.clone(), password_store_dir.clone());
    let (repo_opt6, password_store_dir6) = (repo_opt.clone(), password_store_dir.clone());
    let (repo_opt7, password_store_dir7) = (repo_opt.clone(), password_store_dir.clone());
    let (repo_opt8, password_store_dir8) = (repo_opt.clone(), password_store_dir.clone());
    let (repo_opt9, password_store_dir9) = (repo_opt.clone(), password_store_dir.clone());
    let repo_opt10 = repo_opt.clone();
    let repo_opt11 = repo_opt.clone();
    let repo_opt12 = repo_opt.clone();
    let repo_opt13 = repo_opt.clone();

    ui.add_global_callback(Event::CtrlChar('y'), copy);
    ui.add_global_callback(Key::Enter, copy);
    ui.add_global_callback(Key::Del, move |ui: &mut Cursive| {
        delete(ui, repo_opt2.clone())
    });

    // Movement
    ui.add_global_callback(Event::CtrlChar('n'), down);
    ui.add_global_callback(Event::CtrlChar('p'), up);
    ui.add_global_callback(Event::Key(cursive::event::Key::PageDown), page_down);
    ui.add_global_callback(Event::Key(cursive::event::Key::PageUp), page_up);

    // View list of persons that have access
    ui.add_global_callback(Event::CtrlChar('v'), move |ui: &mut Cursive| {
        view_recipients(ui, repo_opt3.clone(), password_store_dir2.clone())
    });

    // Query editing
    let passwords_clone = std::sync::Arc::clone(&passwords);
    ui.add_global_callback(Event::CtrlChar('w'), move |ui: &mut Cursive| {
        do_delete_last_word(ui, passwords_clone.clone());
    });

    // Editing
    ui.add_global_callback(Event::CtrlChar('o'), move |ui: &mut Cursive| {
        open(ui, repo_opt4.clone(), password_store_dir3.clone())
    });
    let passwords_git_pull_clone = std::sync::Arc::clone(&passwords);
    ui.add_global_callback(Event::CtrlChar('f'), move |ui: &mut Cursive| {
        git_pull(ui, passwords_git_pull_clone.clone(), repo_opt5.clone(), password_store_dir4.clone())
    });
    ui.add_global_callback(Event::CtrlChar('g'), move |ui: &mut Cursive| {
        git_push(ui, repo_opt6.clone())
    });
    ui.add_global_callback(Event::Key(cursive::event::Key::Ins), move |ui: &mut Cursive| {
        create(ui, repo_opt7.clone(), password_store_dir5.clone())
    });

    ui.add_global_callback(Event::Key(cursive::event::Key::Esc), |s| s.quit());

    ui.load_toml(include_str!("../res/style.toml")).unwrap();
    let passwords_clone = std::sync::Arc::clone(&passwords);
    let searchbox = EditView::new()
        .on_edit(move |ui: &mut cursive::Cursive, query, _| {
            search(&passwords_clone, ui, query)
        }).with_id("searchbox")
        .fixed_width(72);

    // Override shortcuts on search box
    let searchbox = OnEventView::new(searchbox)
        .on_event(Key::Up, up)
        .on_event(Key::Down, down);

    let results = SelectView::<pass::PasswordEntry>::new()
        .with_id("results")
        .full_height();

    ui.add_layer(
        LinearLayout::new(Orientation::Vertical)
            .child(
                Dialog::around(
                    LinearLayout::new(Orientation::Vertical)
                        .child(searchbox)
                        .child(results)
                        .fixed_width(72),
                ).title("Ripasso"),
            ).child(
                LinearLayout::new(Orientation::Horizontal)
                    .child(TextView::new(CATALOG.gettext("F1: Menu | ")))
                    .child(TextView::new("").with_id("status_bar"))
                    .full_width(),
            ),
    );

    let passwords_git_pull_clone2 = std::sync::Arc::clone(&passwords);
    ui.menubar()
        .add_subtree(CATALOG.gettext("Operations"),
                     MenuTree::new()
                         .leaf(CATALOG.gettext("Copy (ctrl-y)"), copy)
                         .leaf(CATALOG.gettext("Create (ins) "), move |ui: &mut Cursive| {
                             create(ui, repo_opt8.clone(), password_store_dir6.clone())
                         })
                         .leaf(CATALOG.gettext("Open (ctrl-o)"), move |ui: &mut Cursive| {
                             open(ui, repo_opt9.clone(), password_store_dir7.clone())
                         })
                         .leaf(CATALOG.gettext("Delete (del)"), move |ui: &mut Cursive| {
                             delete(ui, repo_opt10.clone())
                         })
                         .leaf(CATALOG.gettext("Team Members (ctrl-v)"), move |ui: &mut Cursive| {
                             view_recipients(ui, repo_opt11.clone(), password_store_dir8.clone())
                         })
                         .delimiter()
                         .leaf(CATALOG.gettext("Git Pull (ctrl-f)"), move |ui: &mut Cursive| {
                             git_pull(ui, passwords_git_pull_clone2.clone(), repo_opt12.clone(), password_store_dir9.clone())
                         })
                         .leaf(CATALOG.gettext("Git Push (ctrl-g)"), move |ui: &mut Cursive| {
                             git_push(ui, repo_opt13.clone())
                         })
                         .delimiter()
                         .leaf(CATALOG.gettext("Quit (esc)"), |s| s.quit()));

    ui.add_global_callback(Key::F1, |s| s.select_menubar());

    // This construction is to make sure that the password list is populated when the program starts
    // it would be better to signal this somehow from the library, but that got tricky
    thread::sleep(time::Duration::from_millis(200));
    search(&passwords, &mut ui, "");

    ui.run();
}
