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

pub struct rowreader<R> {
    readlen: uint,
    delim: char,
    quote: char,
    f : ~R,
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


fn new_reader<R: Reader>(f: ~R, delim: char, quote: char) -> ~rowreader<~Reader> {
    new_reader_readlen(f, delim, quote, 1024u)
}

fn new_reader_readlen<R: Reader>(f: ~R, delim: char, quote: char, rl: uint) -> ~rowreader<~Reader> {
    ~rowreader {
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
    let mut r : ~[char] = ~[];
    r.reserve(escaped.len());
    let mut in_q = false;
    for &c in escaped.iter() {
        if in_q {
            assert!(c == quote);
            in_q = false;
        } else {
            in_q = c == quote;
            r.push(c);
        }
    }
    r
}

trait rowiter {
  fn readrow(&mut self) -> ~[~str];
  //fn iter(&mut self, f: fn(&row: ~[~str]) -> bool);
}

impl<R:Reader> rowiter for rowreader<~R> {
    #[inline]
    fn readrow(&mut self) -> ~[~str] {
        let mut row:~[~str] = ~[];
        self.state = fieldstart(false);
        let mut do_read = self.buffers.len() == 0u;
        //*row = ~[];
        while !self.terminating {
            if do_read {
                let bytes: ~[u8] = self.f.read_bytes(self.readlen);
                let mut data:~[char] = ~[]; // = (str::from_utf8(self.f.read_bytes(self.readlen))).chars().collect();
                let mut chars = str::from_utf8(bytes).chars();
                if chars.len() == 0u {
                    if !self.trailing_nl {
                        self.terminating = true;
                        data = ~['\n'];
                    } else {
                        return row;
                    }
                }
                // this is horrible, but it avoids the whole parser needing 
                // to know about \r.
                let mut fl = chars.filter( |&c| c != '\r' );
                data = fl.collect();
                //data  = fl.collect::<~[char]>();
                let data_len = data.len();
                if data_len == 0u {
                    continue;
                }

                self.trailing_nl = data[data_len - 1u] == '\n';
                self.buffers.push(data);
                self.offset = 0u;
            }

            let temp = row_from_buf(self);
            if (temp.len() != 0) {
                //vec::append(row, temp);
                for el in temp.iter() {
                  row.push(el.clone());
                }
                /*let buflen = self.buffers.len();
                if buflen > 1u {
                    self.buffers = ~[self.buffers[buflen-1u]];
                }*/
                return row;
            }
            do_read = true;
        }
        return ~[];
    }

    /*fn iter(&mut self, f: fn(mut r: ~[~str]) -> bool) {
        let mut row =  ~[];
        while(true) {
          let temp = self.readrow();
          if(temp.len() == 0) {
            break;
          }
          row = vec::append(row, temp);
          if !f(row) {
              break;
          }
        }
    }*/
}

fn row_from_buf(current: &mut rowreader<~Reader>) -> ~[~str] {
  let mut fields:~[~str] = ~[];
  let cbuffer = current.buffers.len() - 1u;
  let buf = &current.buffers[cbuffer];
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
                      fields.push(decode(&current.buffers, emptyfield, current.quote));
                  }
                  return fields;
              } else if c == current.delim {
                  current.state = fieldstart(true);
                  fields.push(decode(&current.buffers, emptyfield, current.quote));
              } else {
                  current.state = infield(cbuffer, coffset);
              }
          }
          infield(b,o) => {
              debug!("field : {} {}", b, o);
              if c == '\n' {
                 fields.push(decode(&current.buffers, new_bufferfield(current, false, b, o, coffset), current.quote));
                  return fields;
              } else if c == current.delim {
                  current.state = fieldstart(true);
                  fields.push(decode(&current.buffers, new_bufferfield(current, false, b, o, coffset), current.quote));
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
                  fields.push(decode(&current.buffers, new_bufferfield(current, true, b, o, coffset), current.quote));
                  return fields;
              } else if c == current.quote {
                  current.state = inquotedfield(b, o);
              } else if c == current.delim {
                  current.state = fieldstart(true);
                  fields.push(decode(&current.buffers, new_bufferfield(current, true, b, o, coffset), current.quote));
              }
              // swallow odd chars, eg. space between field and "
          }
      }
      debug!("now {}", statestr(current.state));
  }
  return ~[];
}

fn new_bufferfield(cur: &rowreader<~Reader>, escaped: bool, sb: uint, so: uint, eo: uint) -> fieldtype {
    let mut eb = cur.buffers.len() - 1u;
    let mut sb = sb;
    let mut so = so;
    let mut eo = eo;
    if escaped {
        so += 1u;
        if so > cur.buffers[sb].len() {
            sb += 1u;
            so = cur.buffers[sb].len() - 1u;
        }
        if eo > 0u {
            eo -= 1u;
        } else {
            eb -= 1u;
            eo = cur.buffers[eb].len() - 1u;
        }
    }
    bufferfield( bufferdescr{ escaped: escaped, sb: sb, eb: eb, start: so, end: eo })
}

fn decode(bufs: & ~[~[char]], field: fieldtype, quote: char) -> ~str {
    let mut buffers = & bufs;
    match field {
        emptyfield() => { ~"" }
        bufferfield(desc) => {
            let mut buf = ~[];
            buf.reserve(256u);
            let mut i = desc.sb;
            while i <= desc.eb {
                let from = if (i == desc.sb)
                    { desc.start } else { 0u };
                let to = if (i == desc.eb)
                    { desc.end } else { (buffers[i]).len() };
                let mut j = from;
                while j < to {
                    buf = vec::append(buf, [buffers[i][j]]);
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

