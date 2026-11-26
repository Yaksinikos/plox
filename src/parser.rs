////////////////////////////////////////////////////////////////////////

use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Error, Read, Result, Seek, SeekFrom};
use std::path::Path;

use byteorder::ReadBytesExt;
use log::{debug, info, warn};

use crate::{
    ESupportedGame, PluginData, TParser,
    expressions::{
        ALL, ANY, Atomic, DESC, EGVerOperator, EVerOperator, Expression, GVER, NOT, SIZE, VER,
    },
    rules::{
        Conflict, EOrderRule, ERule, EWarningRule, NearEnd, NearStart, Note, Order, Patch,
        Requires, TWarningRule,
    },
};

#[must_use]
pub fn get_parser(game: ESupportedGame, game_version: Option<String>) -> Parser {
    match game {
        ESupportedGame::Morrowind => Parser::new(
            vec![".esp".into(), ".esm".into()],
            ESupportedGame::Morrowind,
            game_version,
        ),
        ESupportedGame::Openmw => Parser::new(
            vec![
                ".esp".into(),
                ".esm".into(),
                ".omwgame".into(),
                ".omwaddon".into(),
                ".omwscripts".into(),
            ],
            ESupportedGame::Openmw,
            game_version,
        ),
        ESupportedGame::Cyberpunk => Parser::new(
            vec![".archive".into()],
            ESupportedGame::Cyberpunk,
            game_version,
        ),
    }
}

#[must_use]
pub fn new_cyberpunk_parser() -> Parser {
    Parser::new(vec![".archive".into()], ESupportedGame::Cyberpunk, None)
}

#[must_use]
pub fn new_tes3_parser() -> Parser {
    Parser::new(
        vec![".esp".into(), ".esm".into()],
        ESupportedGame::Morrowind,
        None,
    )
}

#[must_use]
pub fn new_openmw_parser() -> Parser {
    Parser::new(
        vec![
            ".esp".into(),
            ".esm".into(),
            ".omwgame".into(),
            ".omwaddon".into(),
            ".omwscripts".into(),
        ],
        ESupportedGame::Openmw,
        None,
    )
}

#[derive(Debug)]
struct ChunkWrapper {
    data: Vec<u8>,
    info: String,
}

impl ChunkWrapper {
    const fn new(data: Vec<u8>, info: String) -> Self {
        Self { data, info }
    }
}

#[derive(Debug, Clone)]
pub struct Warning {
    pub rule: EWarningRule,
}

impl Warning {
    #[must_use]
    pub fn get_comment(&self) -> String {
        self.rule.get_comment()
    }

    #[must_use]
    pub fn get_ui_comment(&self) -> String {
        self.rule.get_ui_comment()
    }

    #[must_use]
    pub fn get_plugins(&self) -> Vec<String> {
        self.rule.get_plugins()
    }

    #[must_use]
    pub fn get_rule_name(&self) -> String {
        match self.rule {
            EWarningRule::Conflict(_) => "Conflict".to_owned(),
            EWarningRule::Note(_) => "Note".to_owned(),
            EWarningRule::Patch(_) => "Patch".to_owned(),
            EWarningRule::Requires(_) => "Requires".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Parser {
    pub game: ESupportedGame,
    pub game_version: Option<String>,
    pub ext: Vec<String>,

    pub order_rules: Vec<EOrderRule>,
    pub warning_rules: Vec<EWarningRule>,
    pub warnings: Vec<Warning>,
}

impl Parser {
    #[must_use]
    pub const fn new(ext: Vec<String>, game: ESupportedGame, game_version: Option<String>) -> Self {
        Self {
            ext,
            game,
            game_version,
            warning_rules: vec![],
            order_rules: vec![],
            warnings: vec![],
        }
    }

    /// Evaluates all warning rules and stores a copy of them in self
    /// Retrieve them with self.warnings
    pub fn evaluate_plugins(&mut self, plugins: &[PluginData]) {
        // lowercase all plugin names
        let mods_cpy: Vec<_> = plugins
            .iter()
            .map(|f| {
                let mut x = f.clone();
                let name_lc = x.name.to_lowercase();
                x.name = name_lc;
                x
            })
            .collect();

        let mut result = vec![];
        for rule in &mut self.warning_rules {
            if rule.eval(&mods_cpy) {
                result.push(Warning { rule: rule.clone() });
            }
        }

        self.warnings = result;
    }

    /// Parse rules for a specific game from a file and stores them in self.
    ///
    /// # Errors
    ///
    /// This function will return an error if file io or parsing fails
    pub fn init_from_file<P>(&mut self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        if !path.as_ref().exists() {
            warn!("Could not find rules file {}", path.as_ref().display());
            return Ok(());
        }

        let rules = self.parse_rules_from_path(&path)?;
        info!(
            "Parsed file {} with {} rules",
            path.as_ref().display(),
            rules.len()
        );

        for r in rules {
            match r {
                ERule::EOrderRule(o) => {
                    self.order_rules.push(o);
                }
                ERule::EWarningRule(w) => {
                    self.warning_rules.push(w);
                }
            }
        }

        Ok(())
    }

    /// Parse rules for a specific game, expects the path to be the rules directory
    ///
    /// # Errors
    ///
    /// This function will return an error if file io or parsing fails
    pub fn parse<P>(&mut self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.warning_rules.clear();
        self.order_rules.clear();

        let rules_files = match self.game {
            ESupportedGame::Morrowind | ESupportedGame::Openmw => {
                ["mlox_base.txt", "mlox_user.txt", "mlox_my_rules.txt"].as_slice()
            }
            ESupportedGame::Cyberpunk => ["plox_base.txt", "plox_my_rules.txt"].as_slice(),
        };

        for file in rules_files {
            let path = path.as_ref().join(file);
            self.init_from_file(path)?;
        }

        info!(
            "Parser initialized with {} order rules",
            self.order_rules.len()
        );
        info!(
            "Parser initialized with {} warning rules",
            self.warning_rules.len()
        );
        Ok(())
    }

    /// Parse rules from a rules file
    ///
    /// # Errors
    ///
    /// This function will return an error if file io or parsing fails
    fn parse_rules_from_path<P>(&self, path: P) -> Result<Vec<ERule>>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let rules = self.parse_rules_from_reader(reader)?;
        Ok(rules)
    }

    /// Parse rules from a reader
    ///
    /// # Errors
    ///
    /// This function will return an error if parsing fails
    pub fn parse_rules_from_reader<R>(&self, reader: R) -> Result<Vec<ERule>>
    where
        R: Read + BufRead + Seek,
    {
        // pre-parse into rule blocks
        let mut chunks: Vec<ChunkWrapper> = vec![];
        let mut chunk: Option<ChunkWrapper> = None;
        for (idx, line) in reader.lines().map_while(Result::ok).enumerate() {
            // ignore comments
            if line.trim_start().starts_with(';') {
                continue;
            }
            // lowercase all
            let lower = line.to_lowercase();

            // trim inline comments
            let line = lower
                .find(';')
                .map_or(lower.as_str(), |index| &lower[..index])
                .trim_end()
                .to_owned();

            // we skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // we are inside a chunk
            if chunk.is_some() && new_rule(&line) {
                // end current chunk
                if let Some(chunk) = chunk.take() {
                    chunks.push(chunk);
                }
            }

            // read to current chunk, preserving newline delimeters
            let delimited_line = line + "\n";
            if let Some(chunk) = &mut chunk {
                chunk.data.extend(delimited_line.as_bytes());
            } else {
                chunk = Some(ChunkWrapper::new(
                    delimited_line.as_bytes().to_vec(),
                    (idx + 1).to_string(),
                ));
            }
        }
        // parse last chunk
        if let Some(chunk) = chunk.take() {
            chunks.push(chunk);
        }

        // process chunks
        let mut rules: Vec<ERule> = vec![];
        for (idx, chunk) in chunks.into_iter().enumerate() {
            let info = &chunk.info;

            let cursor = Cursor::new(&chunk.data);
            match self.parse_chunk(cursor) {
                Ok(it) => {
                    rules.push(it);
                }
                Err(err) => {
                    // log error and skip chunk
                    debug!("Error '{err}' at chunk #{idx}, starting at line: {info}");
                    debug!("{}", String::from_utf8_lossy(&chunk.data));
                }
            }
        }

        Ok(rules)
    }

    /// Parses on rule section. Note: Order rules are returned as vec
    ///
    /// # Errors
    ///
    /// This function will return an error if parsing fails
    fn parse_chunk<R>(&self, mut reader: R) -> Result<ERule>
    where
        R: Read + BufRead + Seek,
    {
        // read first char
        let start = reader.read_u8()? as char;
        match start {
            '[' => {
                // start parsing
                // read until the end of the rule expression: e.g. [NOTE comment] body
                if let Ok((mut rule_expression, ruletype)) = parse_rule_expression(&mut reader) {
                    rule_expression.pop();
                    let mut rule: ERule;
                    // parse rule name
                    {
                        if rule_expression.strip_prefix("order").is_some() {
                            rule = Order::default().into();
                        } else if rule_expression.strip_prefix("nearstart").is_some() {
                            rule = NearStart::default().into();
                        } else if rule_expression.strip_prefix("nearend").is_some() {
                            rule = NearEnd::default().into();
                        } else if let Some(rest) = rule_expression.strip_prefix("note") {
                            let mut x = Note::default();
                            x.set_comment(rest.trim().to_owned());
                            rule = x.into();
                        } else if let Some(rest) = rule_expression.strip_prefix("conflict") {
                            let mut x = Conflict::default();
                            x.set_comment(rest.trim().to_owned());
                            rule = x.into();
                        } else if let Some(rest) = rule_expression.strip_prefix("requires") {
                            let mut x = Requires::default();
                            x.set_comment(rest.trim().to_owned());
                            rule = x.into();
                        } else if let Some(rest) = rule_expression.strip_prefix("patch") {
                            let mut x = Patch::default();
                            x.set_comment(rest.trim().to_owned());
                            rule = x.into();
                        } else {
                            // unknown rule, skip
                            return Err(Error::other("Parsing error: unknown rule"));
                        }
                    }

                    // parse body
                    match ruletype {
                        ERuleType::Inline => {
                            // inline rules don't have comments, we just parse the resst of the chunk
                            // now parse rule body
                            ERule::parse(&mut rule, reader, self)?;
                        }
                        ERuleType::Multiline => {
                            // construct the body out of each line with comments trimmed
                            let mut is_first_line = false;
                            let mut comment = String::new();
                            let mut body = String::new();
                            for (idx, line) in reader
                                .lines()
                                .map_while(Result::ok)
                                .filter(|p| !p.trim().is_empty())
                                .enumerate()
                            {
                                if idx == 0 {
                                    is_first_line = true;
                                }

                                // check for those darned comments
                                if is_first_line {
                                    if let Some(first_char) = line.chars().next()
                                        && first_char.is_ascii_whitespace()
                                    {
                                        comment += line.as_str();
                                        continue;
                                    }

                                    if !comment.is_empty() {
                                        if let ERule::EWarningRule(w) = &mut rule {
                                            w.set_comment(comment.clone().trim().into());
                                        }
                                        comment.clear();
                                    }

                                    is_first_line = false;
                                }

                                // this is a proper line
                                body += format!("{line}\n").as_str();
                            }

                            // now parse rule body
                            let body = body.trim();
                            let body_cursor = Cursor::new(body);
                            ERule::parse(&mut rule, body_cursor, self)?;
                        }
                    }
                    Ok(rule)
                } else {
                    Err(Error::other("Parsing error: unknown rule"))
                }
            }
            _ => {
                // error
                Err(Error::other("Parsing error: Not a rule start"))
            }
        }
    }

    #[must_use]
    pub fn ends_with_vec(&self, current_buffer: &str) -> bool {
        self.ext.iter().any(|ext| current_buffer.ends_with(ext))
    }
    fn ends_with_vec_whitespace(&self, current_buffer: &str) -> bool {
        let mut b = false;
        for ext in &self.ext {
            if current_buffer.ends_with(format!("{ext} ").as_str()) {
                b = true;
                break;
            }
        }

        b
    }
    fn ends_with_vec_whitespace_or_newline(&self, current_buffer: &str) -> bool {
        let mut b = false;
        for ext in &self.ext {
            if current_buffer.ends_with(format!("{ext} ").as_str())
                || current_buffer.ends_with(format!("{ext}\n").as_str())
            {
                b = true;
                break;
            }
        }

        b
    }

    /// Splits a String into string tokens (either separated by extension or wrapped in quotation marks)
    #[must_use]
    pub fn tokenize(&self, line: &str) -> Vec<String> {
        let mut tokens: Vec<String> = vec![];

        let mut is_quoted = false;
        let mut current_token: String = String::new();
        for c in line.chars() {
            // check quoted and read in chars
            if c == '"' {
                // started a quoted segment
                if is_quoted {
                    is_quoted = false;
                    // end token
                    tokens.push(current_token.trim().to_owned());
                    current_token.clear();
                } else {
                    is_quoted = true;
                }
                continue;
            }
            current_token += c.to_string().as_str();

            // check if we found an end
            if self.ends_with_vec_whitespace(&current_token) {
                // ignore whitespace in quoted segments
                if is_quoted {
                    current_token += c.to_string().as_str();
                } else {
                    // end token
                    if !current_token.is_empty() {
                        tokens.push(current_token.trim().to_owned());
                        current_token.clear();
                    }
                }
            }
        }

        if !current_token.is_empty() {
            tokens.push(current_token.trim().to_owned());
        }

        tokens
    }

    /// Parses all expressions from a buffer until EOF is reached
    ///
    /// # Errors
    ///
    /// This function will return an error if parsing fails anywhere
    pub fn parse_expressions<R>(&self, mut reader: R) -> Result<Vec<Expression>>
    where
        R: Read + BufRead,
    {
        let mut buffer = vec![];
        reader.read_to_end(&mut buffer)?;

        // pre-parse expressions into chunks
        let mut chunks: Vec<(String, bool)> = vec![];
        let mut current_buffer: String = String::new();
        let mut is_expr = false;
        let mut is_token = false;
        let mut depth = 0;

        for b in buffer {
            if is_expr {
                // if parsing an expression, just count brackets and read the rest into the buffer
                if b == b'[' {
                    depth += 1;
                } else if b == b']' {
                    depth -= 1;
                }
                current_buffer += &(b as char).to_string();

                // check if really an expression
                // valid expressions are [ANY], [ALL], [NOT], [DESC], [SIZE], [VER], [GVER]

                if depth == 0 {
                    // TODO get the list from the traits
                    // we reached the end of the current expression
                    let trimmed = current_buffer.trim();
                    is_expr = false;
                    if EXPRESSIONS
                        .iter()
                        .any(|e| starts_with_whitespace(trimmed, e))
                    {
                        chunks.push((trimmed.to_owned(), true));
                        current_buffer.clear();
                    } else {
                        // not a valid expression, move into token
                        is_token = true;
                    }
                }
            } else if is_token {
                // if parsing tokens, check when ".archive" was parsed into the buffer and end
                current_buffer += &(b as char).to_string();

                if self.ends_with_vec_whitespace_or_newline(&current_buffer) {
                    is_token = false;
                    chunks.push((current_buffer[..current_buffer.len() - 1].to_owned(), false));
                    current_buffer.clear();
                }
            } else {
                // this marks the beginning
                if b == b'[' {
                    // start an expression
                    is_expr = true;
                    depth += 1;
                }
                // ignore whitespace
                else if !b.is_ascii_whitespace() {
                    is_token = true;
                }
                current_buffer += &(b as char).to_string();
            }
        }

        // rest
        if !current_buffer.is_empty() {
            chunks.push((current_buffer.clone(), is_expr));
            current_buffer.clear();
        }

        chunks = chunks
            .iter()
            .map(|f| (f.0.trim().to_owned(), f.1))
            .filter(|p| !p.0.is_empty())
            .collect();

        let mut expressions: Vec<Expression> = vec![];
        for (chunk, is_expr) in chunks {
            {
                let it = self.parse_expression(chunk.as_str(), is_expr)?;
                expressions.push(it);
            }
        }

        Ok(expressions)
    }

    /// Parses `expr` and checks that it is exactly one atomic expression
    fn single_atomic(&self, expr: &str, kind: &str) -> Result<Atomic> {
        let expressions = self.parse_expressions(expr.as_bytes())?;
        if expressions.len() != 1 {
            return Err(Error::other(format!(
                "Parsing error: {kind} expression must have exactly one child expression"
            )));
        }
        match expressions.into_iter().next() {
            Some(Expression::Atomic(atomic)) => Ok(atomic),
            _ => Err(Error::other(format!(
                "Parsing error: {kind} expression must have an atomic child expression"
            ))),
        }
    }

    /// Parses a single expression from a buffer
    ///
    /// # Errors
    ///
    /// This function will return an error if parsing fails
    pub fn parse_expression(&self, reader: &str, is_expression: bool) -> Result<Expression> {
        // an expression may start with
        if !is_expression {
            // is a token
            // in this case just return an atomic
            if !self.ends_with_vec(reader) {
                return Err(Error::other("Parsing error: Not an atomic"));
            }

            return Ok(Atomic::from(reader).into());
        }

        if reader.starts_with('[') {
            // is an expression
            // parse the kind and reurse down
            if let Some(rest) = reader.strip_prefix("[any") {
                let expressions = self.parse_expressions(body_of(rest).as_bytes())?;
                let expr = ANY::new(expressions);
                Ok(expr.into())
            } else if let Some(rest) = reader.strip_prefix("[all") {
                let expressions = self.parse_expressions(body_of(rest).as_bytes())?;
                let expr = ALL::new(expressions);
                Ok(expr.into())
            } else if let Some(rest) = reader.strip_prefix("[not") {
                let expressions = self.parse_expressions(body_of(rest).as_bytes())?;
                expressions.into_iter().last().map_or_else(
                    || Err(Error::other("Parsing error: unknown expression")),
                    |first| Ok(NOT::new(first).into()),
                )
            } else if let Some(rest) = reader.strip_prefix("[desc") {
                let (expr, regex, negated) = parse_desc(body_of(rest)).ok_or_else(unknown)?;
                let atomic = self.single_atomic(&expr, "DESC")?;
                Ok(DESC::new(atomic, regex, negated).into())
            } else if let Some(rest) = reader.strip_prefix("[size") {
                let (expr, size, negated) = parse_size(body_of(rest)).ok_or_else(unknown)?;
                let atomic = self.single_atomic(&expr, "SIZE")?;
                Ok(SIZE::new(atomic, size, negated).into())
            } else if let Some(rest) = reader.strip_prefix("[ver") {
                let (expr, operator, version) = parse_ver(body_of(rest)).ok_or_else(unknown)?;
                let atomic = self.single_atomic(&expr, "VER")?;
                Ok(VER::new(atomic, operator, version.to_string()).into())
            } else if let Some(rest) = reader.strip_prefix("[gver") {
                let (expr, operator, version) = parse_gver(body_of(rest)).ok_or_else(unknown)?;
                let atomic = self.single_atomic(&expr, "GVER")?;
                Ok(GVER::new(atomic, operator, version.to_string()).into())
            } else {
                // unknown expression
                Err(Error::other("Parsing error: unknown expression"))
            }
        } else {
            Err(Error::other("Parsing error: Not an expression"))
        }
    }
}

/// Body of an expression after its keyword, without the closing bracket
fn body_of(rest: &str) -> &str {
    rest.strip_suffix(']').unwrap_or(rest).trim_start()
}

fn unknown() -> Error {
    Error::other("Parsing error: unknown expression")
}

/// Checks if a new rule has started by matching the first chars to the rules names
fn new_rule(line: &str) -> bool {
    [
        "[order",
        "[nearstart",
        "[nearend",
        "[note",
        "[conflict",
        "[requires",
        "[patch",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}

/// Expression keywords
const EXPRESSIONS: [&str; 7] = ["[any", "[all", "[not", "[desc", "[size", "[ver", "[gver"];

fn starts_with_whitespace(current_buffer: &str, arg: &str) -> bool {
    current_buffer.starts_with(format!("{arg} ").as_str())
        || current_buffer.starts_with(format!("{arg}\t").as_str())
}

/// Parses the DESC predicate and returns its parts
fn parse_desc(input: &str) -> Option<(String, String, bool)> {
    //  !/Bite works only with Vampire Embrace/ DW_assassination.esp]
    if let Some(input) = input.strip_prefix("!/") {
        if let Some(end_index) = input.rfind('/') {
            // Extract the substring between "/" and "/"
            let left_part = input[..end_index].trim().to_string();

            // Extract the substring right of the last "/"
            let right_part = input[end_index + 1..].trim().to_string();

            return Some((right_part, left_part, true));
        }
    }
    //  /This version is compatible with Better Robes and Better Clothes./ UFR_v3dot2.esp]
    else if let Some(input) = input.strip_prefix('/')
        && let Some(end_index) = input.rfind('/')
    {
        // Extract the substring between "/" and "/"
        let left_part = input[..end_index].trim().to_string();

        // Extract the substring right of the last "/"
        let right_part = input[end_index + 1..].trim().to_string();

        return Some((right_part, left_part, false));
    }

    None
}

/// Parses the SIZE predicate and returns its parts
fn parse_size(input: &str) -> Option<(String, u64, bool)> {
    // !4921700 Annastia V3.3.esp]
    if let Some(input) = input.strip_prefix('!') {
        if let Some(left_part) = input.split_whitespace().next()
            && let Some(right_part) = input.trim_start().strip_prefix(left_part)
            && let Ok(size) = left_part.parse::<u64>()
        {
            return Some((right_part[1..].to_owned(), size, true));
        }
    }
    // 591786 BMS_Timers_Patch.esp]
    else if let Some(left_part) = input.split_whitespace().next()
        && let Some(right_part) = input.trim_start().strip_prefix(left_part)
        && let Ok(size) = left_part.parse::<u64>()
    {
        return Some((right_part[1..].to_owned(), size, false));
    }

    None
}

/// Parses the VER predicate and returns its parts
fn parse_ver(input: &str) -> Option<(String, EVerOperator, semver::Version)> {
    // >1.51 Rise of House Telvanni.esm
    // = 2.14 Blood and Gore.esp
    // < 3.1 Class Abilities <VER>.esp
    if let Some(input) = input.strip_prefix('<') {
        if let Some(version) = input.split_whitespace().next()
            && let Some(right_part) = input.trim_start().strip_prefix(version)
            && let Ok(semversion) = lenient_semver::parse(version)
        {
            return Some((right_part.to_owned(), EVerOperator::Less, semversion));
        }
    } else if let Some(input) = input.strip_prefix('>') {
        if let Some(version) = input.split_whitespace().next()
            && let Some(right_part) = input.trim_start().strip_prefix(version)
            && let Ok(semversion) = lenient_semver::parse(version)
        {
            return Some((right_part.to_owned(), EVerOperator::Greater, semversion));
        }
    } else if let Some(input) = input.strip_prefix('=')
        && let Some(version) = input.split_whitespace().next()
        && let Some(right_part) = input.trim_start().strip_prefix(version)
        && let Ok(semversion) = lenient_semver::parse(version)
    {
        return Some((right_part.to_owned(), EVerOperator::Equal, semversion));
    }

    None
}

/// Parses the VER predicate and returns its parts
fn parse_gver(input: &str) -> Option<(String, EGVerOperator, semver::Version)> {
    // >1.51 Rise of House Telvanni.esm
    // = 2.14 Blood and Gore.esp
    // < 3.1 Class Abilities <VER>.esp
    if let Some(input) = input.strip_prefix('<') {
        if let Some(version) = input.split_whitespace().next()
            && let Some(right_part) = input.trim_start().strip_prefix(version)
            && let Ok(semversion) = lenient_semver::parse(version)
        {
            return Some((right_part.to_owned(), EGVerOperator::Less, semversion));
        }
    } else if let Some(input) = input.strip_prefix('>') {
        if let Some(version) = input.split_whitespace().next()
            && let Some(right_part) = input.trim_start().strip_prefix(version)
            && let Ok(semversion) = lenient_semver::parse(version)
        {
            return Some((right_part.to_owned(), EGVerOperator::Greater, semversion));
        }
    } else if let Some(input) = input.strip_prefix('=')
        && let Some(version) = input.split_whitespace().next()
        && let Some(right_part) = input.trim_start().strip_prefix(version)
        && let Ok(semversion) = lenient_semver::parse(version)
    {
        return Some((right_part.to_owned(), EGVerOperator::Equal, semversion));
    }

    None
}

pub enum ERuleType {
    Inline,
    Multiline,
}

/// Parses a Rule and comment
///
/// # Errors
///
/// This function will return an error if stream IO fails
fn parse_rule_expression<R>(mut reader: R) -> Result<(String, ERuleType)>
where
    R: Read + BufRead + Seek,
{
    let mut scope = 1;
    let mut buffer = Vec::new();
    let end_index;

    loop {
        let mut byte = [0; 1];
        match reader.read_exact(&mut byte) {
            Ok(()) => {
                buffer.push(byte[0]);

                if byte[0] == b'[' {
                    scope += 1;
                } else if byte[0] == b']' {
                    scope -= 1;
                    if scope == 0 {
                        end_index = buffer.len();
                        break;
                    }
                }
            }
            Err(err) => {
                eprintln!("Error reading input: {err}");
                return Err(err);
            }
        }
    }
    buffer.truncate(end_index);

    // There are only two types of rules allowed
    //
    // Type 1
    // [Rule comment] plugin1.esp plugin2.esp
    // [Rule] plugin1.esp plugin2.esp
    //
    // Type 2
    // [Rule]
    //  comment
    // plugin1.esp
    // plugin2.esp
    // [Rule]
    // plugin1.esp
    // plugin2.esp

    // we don't allow
    // [Rule comment]
    // plugin1.esp
    // plugin2.esp

    // we don't allow
    // [Rule] plugin1.esp
    // plugin2.esp

    // first check if there is a coment inside the rule expression
    let rule_expression = String::from_utf8_lossy(&buffer).trim().to_owned();
    if rule_expression.split_whitespace().nth(1).is_some() {
        // must be inline
        Ok((rule_expression, ERuleType::Inline))
    } else {
        // Undefined, can still be inline but with no comment

        // now read until the next newline
        let idx = reader.stream_position()?;
        buffer.clear();
        reader.read_until(b'\n', &mut buffer)?;
        let rest = String::from_utf8_lossy(&buffer).trim().to_owned();

        if rest.is_empty() {
            // if rest of the line is only whitespace then it's Type 2
            Ok((rule_expression, ERuleType::Multiline))
        } else {
            // if there is any content in the same line as the rule then it's Type 1
            // but we need to seek back
            reader.seek(SeekFrom::Start(idx))?;
            Ok((rule_expression, ERuleType::Inline))
        }
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_parse_rule_expression() -> Result<()> {
        {
            let inputs = [
                ("NOTE comment] more content.", "NOTE comment]"),
                ("NOTE] more content.", "NOTE]"),
                ("NOTE comment]", "NOTE comment]"),
                ("NOTE with [nested] comment]", "NOTE with [nested] comment]"),
                (
                    "NOTE with [nested] comment] and more",
                    "NOTE with [nested] comment]",
                ),
            ];

            for (input, expected) in inputs {
                let reader = Cursor::new(input.as_bytes());
                assert_eq!(expected.to_owned(), parse_rule_expression(reader)?.0);
            }
        }

        {
            let inputs = ["NOTE comment[]", "NOTE comment[with] [[[[[broken scope]"];

            for input in inputs {
                let reader = Cursor::new(input.as_bytes());
                assert!(parse_rule_expression(reader).is_err());
            }
        }

        Ok(())
    }

    #[test]
    fn test_tokenize() {
        let parser = new_cyberpunk_parser();

        let inputs = [
            vec!["a.archive", "my e3.archive.archive"],
            vec![" a.archive", "\"mod with spaces.archive\"", "b.archive"],
            vec![" a.archive", "\"mod with spaces.archive\"", "\"c.archive\""],
            vec!["a mod with spaces.archive"],
            vec!["a.archive"],
        ];

        for input_vec in inputs {
            let input = input_vec.join(" ");
            let expected = input_vec
                .iter()
                .map(|f| f.trim().trim_matches('"').trim())
                .collect::<Vec<_>>();
            assert_eq!(expected, parser.tokenize(&input).as_slice());
        }
    }
}
