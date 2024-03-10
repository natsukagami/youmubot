use serenity::all::MessageBuilder;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Align {
    Left,
    Middle,
    Right,
}

impl Align {
    fn pad(self, input: &str, len: usize) -> String {
        match self {
            Align::Left => format!("{:<len$}", input),
            Align::Middle => format!("{:^len$}", input),
            Align::Right => format!("{:>len$}", input),
        }
    }
}

pub fn table_formatting_unsafe<S: AsRef<str> + std::fmt::Debug, Ss: AsRef<[S]>, Ts: AsRef<[Ss]>>(
    headers: &[&str],
    padding: &[Align],
    table: Ts,
) -> String {
    let table = table.as_ref();
    // get length for each column
    let lens = headers
        .iter()
        .enumerate()
        .map(|(i, header)| {
            table
                .iter()
                .map(|r| r.as_ref()[i].as_ref().len())
                .max()
                .unwrap_or(0)
                .max(header.len())
        })
        .collect::<Vec<_>>();
    // paint with message builder
    let mut m = MessageBuilder::new();
    m.push_line("```");
    // headers first
    for (i, header) in headers.iter().enumerate() {
        if i > 0 {
            m.push(" | ");
        }
        m.push(padding[i].pad(header, lens[i]));
    }
    m.push_line("");
    // separator
    m.push_line(format!(
        "{:-<total$}",
        "",
        total = lens.iter().sum::<usize>() + (lens.len() - 1) * 3
    ));
    // table itself
    for row in table {
        let row = row.as_ref();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                m.push(" | ");
            }
            let cell = cell.as_ref();
            m.push(padding[i].pad(cell, lens[i]));
        }
        m.push_line("");
    }
    m.push("```");
    m.build()
}

pub fn table_formatting<const N: usize, S: AsRef<str> + std::fmt::Debug, Ts: AsRef<[[S; N]]>>(
    headers: &[&'static str; N],
    padding: &[Align; N],
    table: Ts,
) -> String {
    table_formatting_unsafe(headers, padding, table)
}
