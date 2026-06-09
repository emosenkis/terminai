use compact_str::CompactString;

pub trait TermReplySender {
  fn reply(&self, s: CompactString);

  fn host_escape(&self, _s: CompactString) {}
}
