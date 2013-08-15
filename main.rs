extern mod extra;
extern mod syntax;
extern mod rustc;
use std::{io, os};
use std::hashmap::HashSet;
use extra::priority_queue;
use syntax::{ast, codemap};

pub mod words;
mod visitor;

fn get_ast(path: Path) -> (@codemap::CodeMap, @ast::Crate) {
    use rustc::driver::{driver, session};
    use syntax::diagnostic;

    let parsesess = syntax::parse::new_parse_sess(None);
    let input = driver::file_input(path);

    let sessopts = @session::options {
        binary: @"spellck",
        maybe_sysroot: Some(@os::self_exe_path().unwrap().pop()),
        .. (*session::basic_options()).clone()
    };


    let diagnostic_handler = diagnostic::mk_handler(None);
    let span_diagnostic_handler =
        diagnostic::mk_span_handler(diagnostic_handler, parsesess.cm);

    let sess = driver::build_session_(sessopts, parsesess.cm,
                                      diagnostic::emit,
                                      span_diagnostic_handler);

    let mut cfg = driver::build_configuration(sess, @"spellck", &input);
    cfg.push(@codemap::dummy_spanned(ast::MetaWord(@"stage2")));

    let crate = driver::phase_1_parse_input(sess, cfg.clone(), &input);
    (parsesess.cm,
     driver::phase_2_configure_and_expand(sess, cfg, crate))
}

fn main() {
    use extra::getopts;
    use extra::getopts::groups;

    let args = std::os::args();
    let opts = ~[groups::optmulti("d", "dict",
                                  "dictionary file (a list of words, one per line)", "PATH"),
                 groups::optflag("n", "no-def-dict", "don't use the default dictionary"),
                 groups::optflag("h", "help", "show this help message")];

    let matches = groups::getopts(args.tail(), opts).unwrap();
    if getopts::opt_present(&matches, "h") || getopts::opt_present(&matches, "help") {
        println(groups::usage(args[0], opts));
        return;
    }

    let mut words = HashSet::new::<~str>();

    if !(getopts::opt_present(&matches, "n") ||
         getopts::opt_present(&matches, "no-def-dict")) {
        if !read_words_into(&Path("/usr/share/dict/words"), &mut words) {
            return
        }
    }
    for dict in getopts::opt_strs(&matches, "d").move_iter() {
        if !read_words_into(&Path(dict), &mut words) {
            return
        }
    }

    match matches.free {
        [name] => {
            let (cm, crate) = get_ast(Path(name));
            let visitor = @mut visitor::SpellingVisitor::new(words);

            visitor.check_crate(crate);

            struct RevSort {
                sp: codemap::span,
                words: HashSet<~str>
            }
            impl Ord for RevSort {
                fn lt(&self, other: &RevSort) -> bool {
                    self.sp.lo > other.sp.lo ||
                        (self.sp.lo == other.sp.lo && self.sp.hi > other.sp.hi)
                }
            }

            let pq: priority_queue::PriorityQueue<RevSort> =
                do visitor.misspellings.clone().move_iter().map |(k, v)| {
                RevSort { sp: k, words: v }
            }.collect();

            for RevSort {sp, words} in pq.to_sorted_vec().move_iter() {
                let lines = cm.span_to_lines(sp);
                let sp_text = cm.span_to_str(sp);

                let ess = if words.len() == 1 {""} else {"s"};
                printfln!("%s: misspelled word%s: %s", sp_text, ess,
                          // yuck!
                          words.move_iter().to_owned_vec().connect(", "));
                match lines.lines {
                    [line_num, .. _] => {
                        let line = lines.file.get_line(line_num as int);
                        printfln!("%s: %s", sp_text, line);
                    }
                    _ => {}
                }
            }
        }
        _ => {
            io::stderr().write_line(fmt!("Expected exactly one filename"))
        }
    }
}

fn read_words_into<E: Extendable<~str>>
                  (p: &Path, e: &mut E) -> bool {
    match io::file_reader(p) {
        Ok(r) => {
            let r = r.read_lines();
            e.extend(&mut r.move_iter());
            true
        }
        Err(s) => {
            io::stderr().write_line(fmt!("Error reading %s: %s", p.to_str(), s));
            os::set_exit_status(1);
            false
        }
    }
}