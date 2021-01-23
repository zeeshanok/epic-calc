use ansi_escapes::{CursorHide, CursorShow, EraseLines};
use ansi_term::{Color, Style};
use clipboard::{ClipboardContext, ClipboardProvider};
use crossterm_input::{input, InputEvent, KeyEvent, RawScreen, Result};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{stdout, Write};

fn main() -> Result<()> {
    println!(
        "{}Press q to exit, esc to clear, c to copy answer, v to copy expression\n",
        CursorHide
    );

    let _raw = RawScreen::into_raw_mode()?;
    let input = input();
    let mut sync_stdin = input.read_sync();
    let mut string = String::new();

    let grey = Style::new().italic().fg(Color::White).dimmed();
    let mut count = 0u32;
    loop {
        if count != 0 {
            if let Some(event) = sync_stdin.next() {
                match event {
                    InputEvent::Keyboard(KeyEvent::Esc) => {
                        string.clear();
                    }
                    InputEvent::Keyboard(KeyEvent::Backspace) => {
                        let l = string.len();
                        if l != 0 {
                            string.remove(l - 1);
                        }
                    }
                    InputEvent::Keyboard(KeyEvent::Enter) => {
                        string.clear();
                        print!("\n\n\r> ");
                        stdout().flush().unwrap();
                    }
                    InputEvent::Keyboard(ke) => match ke {
                        KeyEvent::Char(c) => {
                            if Expression::qualified(&c) {
                                string.push(c);
                            } else {
                                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                                match c {
                                    'q' => {
                                        stdout().write("\n".as_bytes()).unwrap();
                                        break;
                                    }
                                    'c' => {
                                        if let Some(ans) =
                                            Expression::parse_string(&string).answer()
                                        {
                                            if !string.is_empty() {
                                                ctx.set_contents(ans.to_string()).unwrap();
                                                print!(
                                                    "{}",
                                                    grey.paint(" (Copied answer clipboard)")
                                                );
                                                stdout().flush().unwrap();
                                                continue;
                                            }
                                        }
                                    }
                                    'v' => {
                                        if !string.is_empty() {
                                            ctx.set_contents(string.to_owned()).unwrap();
                                            print!(
                                                "{}",
                                                grey.paint(" (Copied expression clipboard)")
                                            );
                                            stdout().flush().unwrap();
                                            continue;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => continue,
                    },
                    _ => continue,
                };
            }
        }
        count += 1;
        let mut e = Expression::parse_string(&string);
        print!(
            "{}{}\n\r> {}",
            EraseLines(2),
            e.to_syn_high_string(),
            Color::RGB(200, 200, 200).paint(if string.is_empty() {
                Color::White
                    .dimmed()
                    .paint("Enter an expression to evaluate")
                    .to_string()
            } else {
                match e.answer() {
                    Some(num) => {
                        let num = num.to_string();
                        if num != string {
                            num
                        } else {
                            String::new()
                        }
                    }
                    None => String::new(),
                }
            },),
        );
        stdout().flush().unwrap();
    }
    println!("{}", CursorShow);
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum ExpressionPart {
    Number(f64),
    Operation(Operation),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
}

#[derive(Debug, PartialEq, Clone)]
struct Expression {
    raw: String,
    rpn: Vec<ExpressionPart>,
    parts: Vec<ExpressionPart>,
}

impl Operation {
    fn eval(&self, num1: f64, num2: f64) -> f64 {
        match self {
            Operation::Add => num1 + num2,
            Operation::Subtract => num1 - num2,
            Operation::Multiply => num1 * num2,
            Operation::Divide => num1 / num2,
            Operation::Power => num1.powf(num2),
        }
    }

    fn to_str(&self) -> &str {
        match self {
            Operation::Add => "+",
            Operation::Subtract => "-",
            Operation::Multiply => "×",
            Operation::Divide => "÷",
            Operation::Power => "^",
        }
    }
    fn from_str(string: &str) -> Option<Operation> {
        match string {
            "+" => Some(Operation::Add),
            "-" => Some(Operation::Subtract),
            "*" => Some(Operation::Multiply),
            "/" => Some(Operation::Divide),
            "^" => Some(Operation::Power),
            _ => None,
        }
    }
}

impl Expression {
    fn qualified(c: &char) -> bool {
        c.is_numeric()
            || precedence()
                .keys()
                .cloned()
                .collect::<Vec<&str>>()
                .contains(&c.to_string().as_str())
            || *c == '.'
    }

    fn parse_vec(parts_s: &Vec<String>, rpn_s: &Vec<String>, raw: &String) -> Expression {
        let iter = |p: &Vec<String>| {
            let mut parts = Vec::new();
            for i in p.iter() {
                let i = i.trim();
                let part = match i.parse::<f64>() {
                    Ok(num) => ExpressionPart::Number(num),
                    Err(_) => match Operation::from_str(i) {
                        Some(op) => ExpressionPart::Operation(op),
                        None => {
                            continue;
                        }
                    },
                };
                parts.push(part);
            }
            parts
        };

        Expression {
            rpn: (iter)(rpn_s),
            parts: (iter)(parts_s),
            raw: raw.clone(),
        }
    }

    fn parse_string(expr: &String) -> Expression {
        let tester = |c: char| !(c.is_numeric() || [' ', '.', '(', ')'].contains(&c));
        let partitioned_parts = partition(expr, &tester);
        Expression::parse_vec(&partitioned_parts, &postfix(&partitioned_parts), expr)
    }

    fn answer(&mut self) -> Option<f64> {
        let mut rpn: Vec<f64> = Vec::new();
        for i in self.rpn.iter() {
            match i {
                ExpressionPart::Number(num) => {
                    rpn.push(*num);
                }
                ExpressionPart::Operation(op) => {
                    let num2 = rpn.pop()?;
                    let num1 = rpn.pop()?;
                    rpn.push(op.eval(num1, num2));
                }
            }
        }
        Some(*rpn.first()?)
    }

    fn to_syn_high_string(&self) -> String {
        let mut string = String::new();
        for part in self.parts.iter() {
            string.push(' ');
            match part {
                ExpressionPart::Number(num) => {
                    let num = *num;
                    string.push_str(
                        Color::RGB(43, 255, 209)
                            .paint(num.to_string())
                            .to_string()
                            .as_str(),
                    );
                }
                ExpressionPart::Operation(op) => {
                    string.push_str(
                        Color::RGB(255, 43, 244)
                            .paint(op.to_str())
                            .to_string()
                            .as_str(),
                    );
                }
            }
        }
        string.trim().to_string()
    }
}

fn partition<'a>(text: &'a str, tester: &dyn Fn(char) -> bool) -> Vec<String> {
    let mut result = Vec::new();
    let mut last = 0;
    for (index, matched) in text.match_indices(tester) {
        if last != index {
            result.push(text[last..index].to_string());
        }
        result.push(matched.to_string());
        last = index + matched.len();
    }
    if last < text.len() {
        result.push(text[last..].to_string());
    }
    result
}
fn precedence<'a>() -> HashMap<&'a str, u16> {
    [("^", 3), ("/", 2), ("*", 2), ("+", 1), ("-", 1)]
        .iter()
        .cloned()
        .collect()
}
fn postfix(string: &Vec<String>) -> Vec<String> {
    let mut post: Vec<String> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let pre = precedence();
    let ops: Vec<&str> = pre.keys().cloned().collect();
    for c in string.iter() {
        match c.parse::<f64>() {
            Ok(num) => {
                post.push(num.to_string());
            }
            Err(_) => {
                if ops.contains(&c.as_str()) {
                    let pre_op = pre.get(&c.as_str()).unwrap();
                    if stack.len() == 0 {
                        stack.push(c.to_string());
                    } else {
                        while stack.len() > 0 {
                            let top = pre.get(&stack.last().unwrap().as_str()).unwrap();
                            match pre_op.cmp(&top) {
                                Ordering::Equal | Ordering::Less => {
                                    post.push(stack.pop().unwrap());
                                    if stack.len() == 0 {
                                        stack.push(c.to_owned());
                                        break;
                                    }
                                }
                                Ordering::Greater => {
                                    stack.push(c.to_owned());
                                    break;
                                }
                            };
                        }
                    }
                }
            }
        }
    }
    if stack.len() > 0 {
        stack.reverse();
        post.extend(stack);
    }
    post
}
