use std::fmt;

pub trait Width {
    fn width_by_chunk_size(chunk_size: usize) -> usize;
    fn chunk_size(max_width: usize) -> usize {
        let mut chunk_size = 2;
        loop {
            if Self::width_by_chunk_size(chunk_size) > max_width {
                break chunk_size - 1;
            }
            chunk_size += 1;
        }
    }
}

pub trait Stat {
    fn view(&self, max_width: usize) -> Box<dyn StatView + '_>;
}

pub trait StatView: fmt::Display {
    /// header of stat
    fn header(&self) -> &'static str;
    /// body width
    fn width(&self) -> usize;
}

pub trait AddStats {
    /// add stat to `buf`.
    fn add_stats(&self, buf: &mut Stats);
}

#[derive(Default)]
pub struct Stats {
    stats: Vec<Box<dyn Stat>>,
}

impl IntoIterator for Stats {
    type Item = Box<dyn Stat>;

    type IntoIter = <Vec<Box<dyn Stat>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.stats.into_iter()
    }
}

impl Extend<Box<dyn Stat>> for Stats {
    fn extend<T: IntoIterator<Item = Box<dyn Stat>>>(&mut self, iter: T) {
        self.stats.extend(iter)
    }
}

impl Stats {
    pub fn push(&mut self, stat: Box<dyn Stat>) {
        self.stats.push(stat)
    }
}

pub struct StatAllView<'s> {
    views: Vec<Box<dyn StatView + 's>>,
}

impl Stats {
    pub fn view(&self, max_width: usize) -> StatAllView<'_> {
        StatAllView {
            views: self.stats.iter().map(|s| s.view(max_width)).collect(),
        }
    }
}

impl fmt::Display for StatAllView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = self
            .views
            .iter()
            .map(|s| s.header().len().max(s.width()))
            .max()
            .unwrap();
        writeln!(f, "{:-^width$}", " statistics ")?;
        for sv in &self.views {
            writeln!(f, "{}:", sv.header())?;
            writeln!(f, "{}", sv)?;
        }
        write!(f, "{:-<width$}", "")
    }
}
