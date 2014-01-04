#[link(name = "csv", vers = "0.2", uuid = "c88f4e89-fc12-4cb3-a978-35d135aefcfd", author = "grahame")];
#[crate_type = "lib"];

use std::io::{File, Reader};//{writer_util, reader_util};
//use std::map;
use std::hashmap;
use std::result;
use std::str;
use std::vec;

//export rowreader, rowiter,
//       new_reader, new_reader_readlen;

enum state {
    fieldstart(bool),
    infield(uint, uint),
    inquotedfield(uint, uint),
    inquote(uint, uint)
}

pub struct rowreader {
    readlen: uint,
    delim: char,
    quote: char,
    f : ~Reader,
    offset : uint,
    buffers : ~[~[char]],
    state : state,
    trailing_nl : bool,
    terminating : bool
}

struct bufferdescr {
    escaped: bool,
    sb: uint,
    eb: uint,
    start: uint,
    end: uint
}

enum fieldtype {
    emptyfield(),
    bufferfield(bufferdescr)
}

/*iface rowiter {
    fn readrow(&row: [str]) -> bool;
    fn iter(f: fn(&row: [str]) -> bool);
}*/

trait newrowreader {
  fn new_reader(&self, delim: char, quote: char) -> ~rowreader;
  fn new_reader_readlen(&self, delim: char, quote: char, rl: uint) -> ~rowreader;
}

impl newrowreader for rowreader {
  fn new_reader(&self, delim: char, quote: char) -> ~rowreader {
      {
          self.new_reader_readlen(delim, quote, 1024u)
      }
  }

  fn new_reader_readlen(&self, delim: char, quote: char, rl: uint) -> ~rowreader {
      rowreader {
          readlen: rl,
          delim: delim,
          quote: quote,
          f: self,
          offset : 0u,
          buffers : ~[],
          state : fieldstart(false),
          trailing_nl : false,
          terminating: false
      }
  }
}

fn statestr(state: state) -> ~str {
    match state {
        fieldstart(after_delim) => {
            format!("fieldstart : after_delim {}", after_delim)
        }
        infield(b,o) => {
            format!("field : {} {}", b, o)
        }
        inquotedfield(b, o) => {
            format!("inquotedfield : {} {}", b, o)
        }
        inquote(b, o) => {
            format!("inquote : {} {}", b, o)
        }
    }
}

fn unescape(escaped: ~[char], quote: char) -> ~[char] {
    let mut r : [char] = [];
    r.reserve(escaped.len());
    let mut in_q = false;
    for &c in escaped.iter() {
        if in_q {
            assert!(c == quote);
            in_q = false;
        } else {
            in_q = c == quote;
            r += [c];
        }
    }
    r;
}

trait rowiter {
  fn readrow(&row: ~[~str]) -> bool;
}

impl rowiter for rowreader {
    #[inline]
    fn readrow(&self, &row: ~[~str]) -> bool {
        fn row_from_buf(current: rowreader, &fields: ~[~str]) -> bool {
            fn decode(buffers: &mut ~[~[char]], field: fieldtype, quote: char) -> ~str {
                match field {
                    emptyfield() => { "" }
                    bufferfield(desc) => {
                        let mut buf = [];
                        buf.reserve(256u);
                        let mut i = desc.sb;
                        while i <= desc.eb {
                            let from = if (i == desc.sb)
                                { desc.start } else { 0u };
                            let to = if (i == desc.eb)
                                { desc.end } else { (buffers[i]).len() };
                            let mut j = from;
                            while j < to {
                                buf += [buffers[i][j]];
                                j += 1u;
                            }
                            i = i + 1u;
                        }
                        if desc.escaped {
                            buf = unescape(buf, quote);
                        }
                        str::from_utf8(buf)
                    }
                }
            }
            #[inline]
            fn new_bufferfield(current: rowreader, escaped: bool, sb: uint, so: uint, eo: uint) -> fieldtype {
                let mut eb = (*current.buffers).len() - 1u;
                let mut sb = sb;
                let mut so = so;
                let mut eo = eo;
                if escaped {
                    so += 1u;
                    if so > current.buffers[sb].len() {
                        sb += 1u;
                        so = current.buffers[sb].len() - 1u;
                    }
                    if eo > 0u {
                        eo -= 1u;
                    } else {
                        eb -= 1u;
                        eo = current.buffers[eb].len() - 1u;
                    }
                }
                bufferfield( bufferdescr{ escaped: escaped, sb: sb, eb: eb, start: so, end: eo })
            }
            let cbuffer = *current.buffers.len() - 1u;
            let buf = current.buffers[cbuffer];
            while current.offset < buf.len() {
                let coffset = current.offset;
                let c : char = buf[coffset];
                debug!("got '{}' | {}", c, statestr(current.state));
                current.offset += 1u;
                match current.state {
                    fieldstart(after_delim) => {
                        debug!("fieldstart : after_delim {}", after_delim);
                        if c == current.quote {
                            current.state = inquotedfield(cbuffer, coffset);
                        } else if c == '\n' {
                            if after_delim {
                                fields += [decode(current.buffers, emptyfield, current.quote)];
                            }
                            return true;
                        } else if c == current.delim {
                            current.state = fieldstart(true);
                            fields += [decode(current.buffers, emptyfield, current.quote)];
                        } else {
                            current.state = infield(cbuffer, coffset);
                        }
                    }
                    infield(b,o) => {
                        debug!("field : {} {}", b, o);
                        if c == '\n' {
                            fields += [decode(current.buffers, new_bufferfield(current, false, b, o, coffset), current.quote)];
                            return true;
                        } else if c == current.delim {
                            current.state = fieldstart(true);
                            fields += [decode(current.buffers, new_bufferfield(current, false, b, o, coffset), current.quote)];
                        }
                    }
                    inquotedfield(b, o) => {
                        debug!("inquotedfield : {} {}", b, o);
                        if c == current.quote {
                            current.state = inquote(b, o);
                        }
                    }
                    inquote(b, o) => {
                        debug!("inquote : {} {}", b, o);
                        if c == '\n' {
                            fields += [decode(current.buffers, new_bufferfield(current, true, b, o, coffset), current.quote)];
                            return true;
                        } else if c == current.quote {
                            current.state = inquotedfield(b, o);
                        } else if c == current.delim {
                            current.state = fieldstart(true);
                            fields += [decode(current.buffers, new_bufferfield(current, true, b, o, coffset), current.quote)];
                        }
                        // swallow odd chars, eg. space between field and "
                    }
                }
                debug!("now {}", statestr(current.state));
            }
            return false;
        }
        self.state = fieldstart(false);
        let mut do_read = (*self.buffers).len() == 0u;
        row = [];
        while !self.terminating {
            if do_read {
                let mut data = self.f.read_chars(self.readlen);
                if data.len() == 0u {
                    if !self.trailing_nl {
                        self.terminating = true;
                        data = ['\n'];
                    } else {
                        return false;
                    }
                }
                // this is horrible, but it avoids the whole parser needing 
                // to know about \r.
                data = data.filter( |c| c != '\r' );
                let data_len = data.len();
                if data_len == 0u {
                    continue;
                }
                self.trailing_nl = data[data_len - 1u] == '\n';
                *self.buffers += [data];
                self.offset = 0u;
            }

            if row_from_buf(self, row) {
                let buflen = (*self.buffers).len();
                if buflen > 1u {
                    *self.buffers = [self.buffers[buflen-1u]];
                }
                return true;
            }
            do_read = true;
        }
        return false;
    }

    fn iter(&self, f: fn(&row: ~[~str]) -> bool) {
        let mut row = [];
        while self.readrow(row) {
            if !f(row) {
                break;
            }
        }
    }
}

#[cfg(test)]
mod test {
    fn rowmatch(testdata: ~str, expected: ~[~[~str]]) {
        let chk = |s: str, mk: fn(io::reader) -> rowreader| {
            let f = io::str_reader(s);
            let r = mk(f);
            let mut i = 0u;
            let mut row: [str] = [];
            loop {
                if !r.readrow(row) {
                    break;
                }
                let expect = expected[i];
                assert!(row.len() == expect.len());
                let mut j = 0u;
                while j < row.len() {
                    assert!(row[j] == expect[j]);
                    j += 1u;
                }
                i += 1u;
            }
            assert!(i == expected.len());
        };
        let runchecks = |s: str| {
            // test default reader params
            do chk(s) |inp| {
                new_reader_readlen(inp, ',', '"', 2u)
            };
            // test default constructor
            do chk(s) |inp| {
                new_reader(inp, ',', '"')
            };
            // test continuations over read buffers
            let mut j = 1u;
            while j < s.len() {
                do chk(s) |inp| {
                    new_reader_readlen(inp, ',', '"', j)
                };
                j += 1u;
            }
            ret;
        };
        // so we can test trailing newline case, testdata
        // must not end in \n - leave off the last newline
        runchecks(testdata);
        runchecks(str::replace(testdata, "\n", "\r\n"));
        if !str::ends_with(testdata, "\n") {
            runchecks(testdata+"\n");
            runchecks(str::replace(testdata+"\n", "\n", "\r\n"));
        }
    }

    #[test]
    fn simple() {
        rowmatch("a,b,c,d\n1,2,3,4",
                 [["a", "b", "c", "d"], ["1", "2", "3", "4"]]);
    }

    #[test]
    fn trailing_comma() {
        rowmatch("a,b,c,d\n1,2,3,4,",
                 [["a", "b", "c", "d"], ["1", "2", "3", "4", ""]]);
    }

    #[test]
    fn leading_comma() {
        rowmatch("a,b,c,d\n,1,2,3,4",
                 [["a", "b", "c", "d"], ["", "1", "2", "3", "4"]]);
    }

    #[test]
    fn quote_simple() {
        rowmatch("\"Hello\",\"There\"\na,b,\"c\",d",
                 [["Hello", "There"], ["a", "b", "c", "d"]]);
    }

    #[test]
    fn quote_nested() {
        rowmatch("\"Hello\",\"There is a \"\"fly\"\" in my soup\"\na,b,\"c\",d",
                 [["Hello", "There is a \"fly\" in my soup"], ["a", "b", "c", "d"]]);
    }

    #[test]
    fn quote_with_comma() {
        rowmatch("\"1,2\"",
                 [["1,2"]])
    }

    #[test]
    fn quote_with_other_comma() {
        rowmatch("1,2,3,\"a,b,c\"",
                 [["1", "2", "3", "a,b,c"]])
    }

    #[test]
    fn blank_line() {
        rowmatch("\n\n", [[], []]);
    }

    #[test]
    fn iter_test() {
        let f = io::str_reader("a brown,cat");
        let r : rowreader = new_reader(f, ',', '"');
        for row in r.iter() {
            assert!(row[0] == "a brown");
            assert!(row[1] == "cat");
        }
    }
}

