//! Spreadsheet data model with formula support
//!
//! Supports formulas with:
//! - Ranges: =SUM(A1:A10)
//! - Individual cells: =SUM(A1,A3,B2)
//! - Mixed: =SUM(A1:A3,B5,C1:C3)

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

// Thread-local evaluation stack to detect circular references
thread_local! {
    static EVAL_STACK: RefCell<HashSet<(usize, usize)>> = RefCell::new(HashSet::new());
}

pub const MAX_ROWS: usize = 999;
pub const MAX_COLS: usize = 26; // A-Z

/// A cell address like A1, B12
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellAddr {
    pub col: usize, // 0 = A
    pub row: usize, // 0 = 1
}

impl CellAddr {
    pub fn new(col: usize, row: usize) -> Self {
        Self { col, row }
    }

    pub fn label(&self) -> String {
        format!("{}{}", col_letter(self.col), self.row + 1)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_uppercase();
        if s.is_empty() { return None; }
        let col_char = s.chars().next()?;
        if !col_char.is_ascii_uppercase() { return None; }
        let col = (col_char as usize) - ('A' as usize);
        let row: usize = s[1..].parse().ok()?;
        if row == 0 { return None; }
        Some(Self { col, row: row - 1 })
    }
}

pub fn col_letter(col: usize) -> char {
    (b'A' + col as u8) as char
}

/// Raw cell content
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cell {
    pub input: String,
}

/// Evaluated cell value
#[derive(Clone, Debug)]
pub enum CellValue {
    Empty,
    Text(String),
    Number(f64),
    Error(String),
}

impl CellValue {
    pub fn display(&self) -> String {
        match self {
            CellValue::Empty => String::new(),
            CellValue::Text(s) => s.clone(),
            CellValue::Number(n) => {
                if *n == n.floor() && n.abs() < 1e12 {
                    format!("{}", *n as i64)
                } else {
                    format!("{:.4}", n).trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
            CellValue::Error(e) => format!("#ERR: {}", e),
        }
    }
}

/// The spreadsheet
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sheet {
    pub cells: HashMap<CellAddr, Cell>,
    pub path: Option<PathBuf>,
    pub modified: bool,
    pub col_widths: Vec<f32>,
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new()
    }
}

impl Sheet {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
            path: None,
            modified: false,
            col_widths: vec![80.0; MAX_COLS],
        }
    }

    pub fn get_input(&self, col: usize, row: usize) -> &str {
        self.cells
            .get(&CellAddr::new(col, row))
            .map(|c| c.input.as_str())
            .unwrap_or("")
    }

    pub fn set_input(&mut self, col: usize, row: usize, input: String) {
        if input.is_empty() {
            self.cells.remove(&CellAddr::new(col, row));
        } else {
            self.cells.insert(CellAddr::new(col, row), Cell { input });
        }
        self.modified = true;
    }

    /// Evaluate a cell
    pub fn eval(&self, col: usize, row: usize) -> CellValue {
        let input = self.get_input(col, row);
        if input.is_empty() {
            return CellValue::Empty;
        }

        // Check for circular reference
        let cell_key = (col, row);
        let is_circular = EVAL_STACK.with(|stack| {
            stack.borrow().contains(&cell_key)
        });
        if is_circular {
            return CellValue::Error("CIRCULAR".into());
        }

        // Add this cell to the evaluation stack
        EVAL_STACK.with(|stack| {
            stack.borrow_mut().insert(cell_key);
        });

        let result = if let Some(formula) = input.strip_prefix('=') {
            self.eval_formula(formula)
        } else if let Ok(n) = input.parse::<f64>() {
            CellValue::Number(n)
        } else {
            CellValue::Text(input.to_string())
        };

        // Remove this cell from the evaluation stack
        EVAL_STACK.with(|stack| {
            stack.borrow_mut().remove(&cell_key);
        });

        result
    }

    fn eval_formula(&self, formula: &str) -> CellValue {
        let formula = formula.trim().to_uppercase();

        // Handle empty formula
        if formula.is_empty() {
            return CellValue::Empty;
        }

        // Check for incomplete function calls (user still typing)
        let func_names = ["SUM", "AVG", "AVERAGE", "MIN", "MAX", "COUNT", "PRODUCT"];
        for func in &func_names {
            if formula.starts_with(func) {
                let rest = &formula[func.len()..];
                // Check if it's an incomplete function call
                if rest.is_empty() || rest == "(" || (rest.starts_with('(') && !rest.ends_with(')')) {
                    return CellValue::Error("incomplete".into());
                }
            }
        }

        // SUM(...)
        if let Some(inner) = strip_func(&formula, "SUM") {
            return self.eval_multi_func(inner, |vals| vals.iter().sum());
        }
        // AVG / AVERAGE
        if let Some(inner) = strip_func(&formula, "AVG")
            .or_else(|| strip_func(&formula, "AVERAGE"))
        {
            return self.eval_multi_func(inner, |vals| {
                if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 }
            });
        }
        // MIN
        if let Some(inner) = strip_func(&formula, "MIN") {
            return self.eval_multi_func(inner, |vals| {
                vals.iter().cloned().fold(f64::INFINITY, f64::min)
            });
        }
        // MAX
        if let Some(inner) = strip_func(&formula, "MAX") {
            return self.eval_multi_func(inner, |vals| {
                vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            });
        }
        // COUNT
        if let Some(inner) = strip_func(&formula, "COUNT") {
            return self.eval_multi_func(inner, |vals| vals.len() as f64);
        }
        // PRODUCT
        if let Some(inner) = strip_func(&formula, "PRODUCT") {
            return self.eval_multi_func(inner, |vals| {
                if vals.is_empty() { 0.0 } else { vals.iter().product() }
            });
        }

        // Simple arithmetic: cell ref or number +/-/* / cell ref or number
        self.eval_arithmetic(&formula)
    }

    /// Evaluate a function that accepts comma-separated arguments.
    /// Each argument can be:
    ///   - A range like A1:B3
    ///   - An individual cell like A1
    ///   - A number like 42
    fn eval_multi_func(&self, args_str: &str, f: impl Fn(&[f64]) -> f64) -> CellValue {
        match self.collect_multi_args(args_str) {
            Ok(vals) => CellValue::Number(f(&vals)),
            Err(e) => CellValue::Error(e),
        }
    }

    /// Collect numeric values from a comma-separated list of arguments.
    /// Each argument can be a range (A1:B3), a cell reference (A1), or a number.
    fn collect_multi_args(&self, args_str: &str) -> Result<Vec<f64>, String> {
        let mut vals = Vec::new();

        for arg in args_str.split(',') {
            let arg = arg.trim();
            if arg.is_empty() { continue; }

            if arg.contains(':') {
                // It's a range like A1:B3
                let range_vals = self.collect_range(arg)?;
                vals.extend(range_vals);
            } else if let Ok(n) = arg.parse::<f64>() {
                // It's a literal number
                vals.push(n);
            } else if let Some(addr) = CellAddr::parse(arg) {
                // It's a cell reference
                if let CellValue::Number(n) = self.eval(addr.col, addr.row) {
                    vals.push(n);
                }
                // Non-numeric cells are silently skipped (like Excel)
            } else {
                return Err(format!("Bad argument: {}", arg));
            }
        }

        Ok(vals)
    }

    /// Collect numeric values from a range like "A1:B3"
    fn collect_range(&self, range_str: &str) -> Result<Vec<f64>, String> {
        let parts: Vec<&str> = range_str.split(':').collect();
        if parts.len() != 2 {
            return Err("expected range like A1:A10".into());
        }
        let start = CellAddr::parse(parts[0]).ok_or("bad start address")?;
        let end = CellAddr::parse(parts[1]).ok_or("bad end address")?;

        let mut vals = Vec::new();
        let (r0, r1) = (start.row.min(end.row), start.row.max(end.row));
        let (c0, c1) = (start.col.min(end.col), start.col.max(end.col));
        for r in r0..=r1 {
            for c in c0..=c1 {
                if let CellValue::Number(n) = self.eval(c, r) {
                    vals.push(n);
                }
            }
        }
        Ok(vals)
    }

    fn eval_arithmetic(&self, expr: &str) -> CellValue {
        // Try simple binary: A + B, A - B, A * B, A / B
        for &op in &['+', '-', '*', '/'] {
            // Find operator not at start (to allow negative numbers)
            if let Some(pos) = expr[1..].find(op).map(|p| p + 1) {
                let left = self.resolve_value(expr[..pos].trim());
                let right = self.resolve_value(expr[pos + 1..].trim());
                match (left, right) {
                    (Some(a), Some(b)) => {
                        let result = match op {
                            '+' => a + b,
                            '-' => a - b,
                            '*' => a * b,
                            '/' => {
                                if b == 0.0 {
                                    return CellValue::Error("DIV/0".into());
                                }
                                a / b
                            }
                            _ => unreachable!(),
                        };
                        return CellValue::Number(result);
                    }
                    _ => continue,
                }
            }
        }

        // Single cell reference or number
        if let Some(v) = self.resolve_value(expr) {
            CellValue::Number(v)
        } else {
            CellValue::Error(format!("Cannot evaluate: {}", expr))
        }
    }

    fn resolve_value(&self, token: &str) -> Option<f64> {
        let token = token.trim();
        if let Ok(n) = token.parse::<f64>() {
            return Some(n);
        }
        if let Some(addr) = CellAddr::parse(token) {
            if let CellValue::Number(n) = self.eval(addr.col, addr.row) {
                return Some(n);
            }
        }
        None
    }

    /// Used rows (max row index with data + 1)
    pub fn used_rows(&self) -> usize {
        self.cells.keys().map(|a| a.row + 1).max().unwrap_or(0).max(20)
    }

    /// Used cols
    pub fn used_cols(&self) -> usize {
        self.cells.keys().map(|a| a.col + 1).max().unwrap_or(0).max(8)
    }

    pub fn display_title(&self) -> String {
        let name = self.path.as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        if self.modified { format!("{}*", name) } else { name }
    }

    /// Export to CSV
    pub fn save_csv(&mut self, path: &PathBuf) -> Result<(), String> {
        let mut wtr = csv::Writer::from_path(path).map_err(|e| e.to_string())?;
        let rows = self.used_rows();
        let cols = self.used_cols();
        for r in 0..rows {
            let mut record: Vec<String> = Vec::new();
            for c in 0..cols {
                record.push(self.eval(c, r).display());
            }
            wtr.write_record(&record).map_err(|e| e.to_string())?;
        }
        wtr.flush().map_err(|e| e.to_string())?;
        self.path = Some(path.clone());
        self.modified = false;
        Ok(())
    }

    /// Import from CSV
    pub fn open_csv(path: PathBuf) -> Result<Self, String> {
        let mut sheet = Sheet::new();
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(&path)
            .map_err(|e| e.to_string())?;
        for (row, record) in rdr.records().enumerate() {
            let record = record.map_err(|e| e.to_string())?;
            for (col, field) in record.iter().enumerate() {
                if !field.is_empty() && col < MAX_COLS && row < MAX_ROWS {
                    sheet.set_input(col, row, field.to_string());
                }
            }
        }
        sheet.path = Some(path);
        sheet.modified = false;
        Ok(sheet)
    }

    /// Save as JSON
    pub fn save_json(&mut self, path: &PathBuf) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        self.path = Some(path.clone());
        self.modified = false;
        Ok(())
    }

    /// Open from JSON
    pub fn open_json(path: PathBuf) -> Result<Self, String> {
        let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut sheet: Sheet = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        sheet.path = Some(path);
        sheet.modified = false;
        Ok(sheet)
    }
}

fn strip_func<'a>(expr: &'a str, name: &str) -> Option<&'a str> {
    let expr = expr.trim();
    if expr.starts_with(name) && expr.ends_with(')') {
        let rest = &expr[name.len()..];
        if rest.starts_with('(') {
            return Some(&rest[1..rest.len() - 1]);
        }
    }
    None
}
