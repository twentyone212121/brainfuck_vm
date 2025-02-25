use std::io::{self, ErrorKind, Read, Write};

/// Enum representing Brainfuck commands.
/// Jump instructions use command addresses for loop execution.
#[derive(Debug)]
enum Command {
    IncrementDataPointer,
    DecrementDataPointer,
    Increment,
    Decrement,
    WriteByte,
    ReadByte,
    JumpForwardIfZero(CommandAddress),
    JumpBackwardIfNonZero(CommandAddress),
}

type CommandAddress = usize;

/// Enum for possible parsing errors.
/// Currently, it only detects unmatched brackets.
#[derive(Debug)]
enum ParsingError {
    UnmatchedBracket(CommandAddress),
}

/// Parses Brainfuck source code into a vector of `Command` instructions.
/// Ensures that brackets are correctly matched and swaps jump commands accordingly.
fn compile(text: &str) -> Result<Vec<Command>, ParsingError> {
    use self::Command as C;

    let charset = "><+-.,[]";

    let mut brackets_stack = Vec::new();
    let mut brackets_swaps = Vec::new();

    let tokens: Vec<char> = text.chars().filter(|c| charset.contains(*c)).collect();
    let mut commands = Vec::with_capacity(tokens.len());

    for (i, t) in tokens.into_iter().enumerate() {
        let command = match t {
            '>' => C::IncrementDataPointer,
            '<' => C::DecrementDataPointer,
            '+' => C::Increment,
            '-' => C::Decrement,
            '.' => C::WriteByte,
            ',' => C::ReadByte,
            '[' => {
                brackets_stack.push(i);
                C::JumpBackwardIfNonZero(i)
            }
            ']' => {
                if let Some(matching_index) = brackets_stack.pop() {
                    brackets_swaps.push((matching_index, i));
                    C::JumpForwardIfZero(i)
                } else {
                    return Err(ParsingError::UnmatchedBracket(i));
                }
            }
            _ => unreachable!(),
        };
        commands.push(command);
    }

    if !brackets_stack.is_empty() {
        return Err(ParsingError::UnmatchedBracket(brackets_stack[0]));
    }

    for (a, b) in brackets_swaps {
        commands.swap(a, b);
    }

    Ok(commands)
}

/// Executes compiled Brainfuck commands on a memory tape.
/// Handles input/output operations via provided `Read` and `Write` streams.
fn eval_on_tape<R: Read, W: Write>(
    commands: &[Command],
    tape: &mut [u8],
    mut data_pointer: usize,
    mut reader: R,
    mut writer: W,
) -> io::Result<()> {
    use self::Command as C;

    let mut instruction_pointer = 0;

    while instruction_pointer < commands.len() {
        let command = &commands[instruction_pointer];

        match command {
            C::IncrementDataPointer => data_pointer += 1,
            C::DecrementDataPointer => data_pointer -= 1,
            C::Increment => tape[data_pointer] += 1,
            C::Decrement => tape[data_pointer] -= 1,
            C::WriteByte => {
                writer.write(&tape[data_pointer..data_pointer + 1])?;
            }
            C::ReadByte => {
                let mut buf = [0];
                let read = match reader.read_exact(&mut buf) {
                    Ok(()) => buf[0],
                    Err(e) if e.kind() == ErrorKind::UnexpectedEof => 0,
                    e => return e,
                };
                tape[data_pointer] = read;
            }
            C::JumpForwardIfZero(address) => {
                if tape[data_pointer] == 0 {
                    instruction_pointer = *address;
                }
            }
            C::JumpBackwardIfNonZero(address) => {
                if tape[data_pointer] != 0 {
                    instruction_pointer = *address;
                }
            }
        };

        instruction_pointer += 1;
    }

    Ok(())
}

/// Wrapper function to initialize memory and execute a Brainfuck program.
fn eval<R: Read, W: Write>(commands: &[Command], reader: R, writer: W) -> io::Result<()> {
    let mut tape = vec![0; 10_000];
    let data_pointer = tape.len() / 2;
    eval_on_tape(commands, &mut tape, data_pointer, reader, writer)
}

fn main() -> io::Result<()> {
    let Some(source_code) = std::env::args().nth(1) else {
        return Err(io::Error::other(
            "No second argument. Please provide an argument with Brainfuck program as a string.",
        ));
    };

    match compile(&source_code) {
        Ok(program) => eval(&program, std::io::stdin(), std::io::stdout()),
        Err(ParsingError::UnmatchedBracket(index)) => writeln!(
            std::io::stderr(),
            "The program is incorrect. Unmatched bracket at index {index}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test Brainfuck loop [->+<] which transfers a value from one cell to another.
    #[test]
    fn test_eval_add() {
        let mut tape = [1, 2];
        let data_pointer = 0;

        // [->+<]
        let commands = [
            Command::JumpForwardIfZero(5),
            Command::Decrement,
            Command::IncrementDataPointer,
            Command::Increment,
            Command::DecrementDataPointer,
            Command::JumpBackwardIfNonZero(0),
        ];

        let reader = &[0_u8][..];
        let writer = &mut [0_u8][..];

        eval_on_tape(&commands, &mut tape, data_pointer, reader, writer).unwrap();

        assert_eq!(tape[0], 0);
        assert_eq!(tape[1], 1 + 2);
    }

    /// Test full "Hello World!" Brainfuck program.
    #[test]
    fn test_hello_world() {
        let source_code = "++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]\
            >>.>---.+++++++..+++.>>.<-.<.+++.------.--------.>>+.>++.";
        let reader = &[0_u8][..];
        let mut writer: Vec<u8> = Vec::new();

        let program = compile(source_code).unwrap();
        eval(&program, reader, &mut writer).unwrap();

        assert_eq!(writer, "Hello World!\n".as_bytes());
    }

    /// Test simple echo program that copies input to output.
    #[test]
    fn test_cat() {
        let source_code = ">,[>,]<[<]>[.>]";
        let reader = "Hello, World!\0".as_bytes();
        let mut writer: Vec<u8> = Vec::new();

        let program = compile(source_code).unwrap();
        eval(&program, reader, &mut writer).unwrap();

        assert_eq!(writer, reader[..reader.len() - 1]);
    }
}
