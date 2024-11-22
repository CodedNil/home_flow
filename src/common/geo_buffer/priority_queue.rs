pub struct PriorityQueue<T: std::cmp::PartialOrd> {
    size: usize,
    content: Vec<T>,
}

impl<T: std::cmp::PartialOrd> PriorityQueue<T> {
    pub const fn new() -> Self {
        Self {
            size: 0,
            content: Vec::new(),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn insert(&mut self, item: T) {
        self.content.push(item);
        let mut cur = self.size;
        let mut par;
        while cur != 0 {
            par = (cur - 1) / 2;
            if self.content[cur] < self.content[par] {
                self.content.swap(cur, par);
                cur = par;
            } else {
                break;
            }
        }
        self.size += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let ret = self.content.swap_remove(0);
        let mut cur = 0;
        let mut nc;
        self.size -= 1;
        while cur < self.size {
            let lc = cur * 2 + 1;
            let rc = cur * 2 + 2;
            if lc >= self.size {
                break;
            } else if rc >= self.size {
                nc = lc;
            } else {
                nc = if self.content[lc] < self.content[rc] {
                    lc
                } else {
                    rc
                };
            }
            if self.content[nc] < self.content[cur] {
                self.content.swap(cur, nc);
                cur = nc;
            } else {
                break;
            }
        }
        Some(ret)
    }
}
