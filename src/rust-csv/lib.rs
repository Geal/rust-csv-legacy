#[link(name = "csv", vers = "0.2", uuid = "c88f4e89-fc12-4cb3-a978-35d135aefcfd", author = "grahame")];
#[crate_type = "lib"];

use std;
use io::{writer_util, reader_util};
use std::map;
use map::hashmap;
use result;

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
    f : io::reader,
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
          new_reader_readlen(f, delim, quote, 1024u)
      }
  }

  fn new_reader_readlen(&self, delim: char, quote: char, rl: uint) -> ~rowreader {
      rowreader {
          readlen: rl,
          delim: delim,
          quote: quote,
          f: f,
          offset : 0u,
          buffers : ~[],
          state : fieldstart(false),
          trailing_nl : false,
          terminating: false
      }
  }
}

fn statestr(state: state) -> str {
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

fn unescape(escaped: [char], quote: char) -> [char] {
    let mut r : [char] = [];
    vec::reserve(r, vec::len(escaped));
    let mut in_q = false;
    for c in vec::each(escaped) {
        if in_q {
            assert(c == quote);
            in_q = false;
        } else {
            in_q = c == quote;
            r += [c];
        }
    }
    r;
}

impl rowiter for rowreader {
    #[inline]
    fn readrow(&row: [str]) -> bool {
        fn row_from_buf(&self: rowreader, &fields: [str]) -> bool {
            fn decode(buffers: @mut [[char]], field: fieldtype, quote: char) -> str {
                match field {
                    emptyfield() => { "" }
                    bufferfield(desc) => {
                        let mut buf = [];
                        vec::reserve(buf, 256u);
                        let mut i = desc.sb;
                        while i <= desc.eb {
                            let from = if (i == desc.sb)
                                { desc.start } else { 0u };
                            let to = if (i == desc.eb)
                                { desc.end } else { vec::len(buffers[i]) };
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
                        str::from_chars(buf)
                    }
                }
            }
            #[inline]
            fn new_bufferfield(self: rowreader, escaped: bool, sb: uint, so: uint, eo: uint) -> fieldtype {
                let mut eb = vec::len(*self.buffers) - 1u;
                let mut sb = sb;
                let mut so = so;
                let mut eo = eo;
                if escaped {
                    so += 1u;
                    if so > vec::len(self.buffers[sb]) {
                        sb += 1u;
                        so = vec::len(self.buffers[sb]) - 1u;
                    }
                    if eo > 0u {
                        eo -= 1u;
                    } else {
                        eb -= 1u;
                        eo = vec::len(self.buffers[eb]) - 1u;
                    }
                }
                bufferfield( descr{ escaped: escaped, sb: sb, eb: eb, start: so, end: eo })
            }
            let cbuffer = vec::len(*self.buffers) - 1u;
            let buf = self.buffers[cbuffer];
            while self.offset < vec::len(buf) {
                let coffset = self.offset;
                let c : char = buf[coffset];
                debug!("got '%c' | %s", c, statestr(self.state));
                self.offset += 1u;
                match self.state {
                    fieldstart(after_delim) => {
                        debug!("fieldstart : after_delim %b", after_delim);
                        if c == self.quote {
                            self.state = inquotedfield(cbuffer, coffset);
                        } else if c == '\n' {
                            if after_delim {
                                fields += [decode(self.buffers, emptyfield, self.quote)];
                            }
                            return true;
                        } else if c == self.delim {
                            self.state = fieldstart(true);
                            fields += [decode(self.buffers, emptyfield, self.quote)];
                        } else {
                            self.state = infield(cbuffer, coffset);
                        }
                    }
                    infield(b,o) => {
                        debug!("field : %u %u", b, o);
                        if c == '\n' {
                            fields += [decode(self.buffers, new_bufferfield(self, false, b, o, coffset), self.quote)];
                            return true;
                        } else if c == self.delim {
                            self.state = fieldstart(true);
                            fields += [decode(self.buffers, new_bufferfield(self, false, b, o, coffset), self.quote)];
                        }
                    }
                    inquotedfield(b, o) => {
                        debug!("inquotedfield : %u %u", b, o);
                        if c == self.quote {
                            self.state = inquote(b, o);
                        }
                    }
                    inquote(b, o) => {
                        debug!("inquote : %u %u", b, o);
                        if c == '\n' {
                            fields += [decode(self.buffers, new_bufferfield(self, true, b, o, coffset), self.quote)];
                            return true;
                        } else if c == self.quote {
                            self.state = inquotedfield(b, o);
                        } else if c == self.delim {
                            self.state = fieldstart(true);
                            fields += [decode(self.buffers, new_bufferfield(self, true, b, o, coffset), self.quote)];
                        }
                        // swallow odd chars, eg. space between field and "
                    }
                }
                debug!("now %s", statestr(self.state));
            }
            return false;
        }
        self.state = fieldstart(false);
        let mut do_read = vec::len(*self.buffers) == 0u;
        row = [];
        while !self.terminating {
            if do_read {
                let mut data = self.f.read_chars(self.readlen);
                if vec::len(data) == 0u {
                    if !self.trailing_nl {
                        self.terminating = true;
                        data = ['\n'];
                    } else {
                        return false;
                    }
                }
                // this is horrible, but it avoids the whole parser needing 
                // to know about \r.
                data = vec::filter(data, |c| c != '\r' );
                let data_len = vec::len(data);
                if data_len == 0u {
                    cont;
                }
                self.trailing_nl = data[data_len - 1u] == '\n';
                *self.buffers += [data];
                self.offset = 0u;
            }

            if row_from_buf(self, row) {
                let buflen = vec::len(*self.buffers);
                if buflen > 1u {
                    *self.buffers = [self.buffers[buflen-1u]];
                }
                return true;
            }
            do_read = true;
        }
        return false;
    }

    fn iter(f: fn(&row: [str]) -> bool) {
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
    fn rowmatch(testdata: str, expected: [[str]]) {
        let chk = fn@(s: str, mk: fn(io::reader) -> rowreader) {
            let f = io::str_reader(s);
            let r = mk(f);
            let mut i = 0u;
            let mut row: [str] = [];
            loop {
                if !r.readrow(row) {
                    break;
                }
                let expect = expected[i];
                assert(vec::len(row) == vec::len(expect));
                let mut j = 0u;
                while j < row.len() {
                    assert(row[j] == expect[j]);
                    j += 1u;
                }
                i += 1u;
            }
            assert(i == vec::len(expected));
        };
        let runchecks = fn@(s: str) {
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
            while j < str::len(s) {
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
        for r.iter() |row| {
            assert(row[0] == "a brown");
            assert(row[1] == "cat");
        }
    }
}

