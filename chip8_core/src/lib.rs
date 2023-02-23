use rand::random;

pub const SCREEN_WIDTH: usize = 64;
pub const SCREEN_HEIGHT: usize = 32;

const RAM_SIZE: usize = 4096;
const NUM_REGISTERS: usize = 16;
const STACK_SIZE: usize = 16;
const NUM_KEYS: usize = 16;

const START_ADDR: u16 = 0x200; // chip8 convention starts programs at 0x200, chip8 program takes up the first part of ram

const FONTSET_SIZE: usize = 80;
const FONTSET: [u8; FONTSET_SIZE] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80 // F
];
// representation of 1 as above, each hex is translated to a byte (a row)
// 00100000 = 0x20
// 01100000 = 0x60
// 00100000 = 0x20
// 00100000 = 0x20
// 01110000 = 0x70

pub struct Emulator {
    pc: u16, // special register program counter, keep track of idx of current instruction
    ram: [u8; RAM_SIZE], // create ram which is 4096 bytes
    display: [bool; SCREEN_WIDTH * SCREEN_HEIGHT], // chip8 keeps screen state, 1 bit black/white (monochrome)
    v_registers: [u8; NUM_REGISTERS], // chip8 uses 16 v registers instead of RAM to speed game execution up
    i_register: u16, // i register used to index RAM
    stack_ptr: u16, // points to the top of the stack
    stack: [u16; STACK_SIZE], // stack implemented as a static array
    keys: [bool; NUM_KEYS],
    delay_t: u8, // delay timer, performs action when 0, counts down every cycle
    sound_t: u8 // chip8 emits a sound when 0, counts down every cycle
}
impl Emulator {
    // init operations
    pub fn new() -> Self {
        let mut new_emulator = Self {
            pc: START_ADDR,
            ram: [0; RAM_SIZE], // start all RAM 0
            display: [false; SCREEN_WIDTH * SCREEN_HEIGHT], // start all pixels off, black, false, 0
            v_registers: [0; NUM_REGISTERS], // init v_registers with blank
            i_register: 0,
            stack_ptr: 0,
            stack: [0; STACK_SIZE],
            keys: [false; NUM_KEYS],
            delay_t: 0,
            sound_t: 0
        };
        new_emulator.ram[..FONTSET_SIZE].copy_from_slice(&FONTSET); // load the fontsize into ram by replacing idx 0 up to FONTSET_SIZE as FONTSET

        new_emulator
    }

    pub fn reset(&mut self) {
        self.pc = START_ADDR;
        self.ram = [0; RAM_SIZE];
        self.display = [false; SCREEN_WIDTH * SCREEN_HEIGHT];
        self.v_registers = [0; NUM_REGISTERS];
        self.i_register = 0;
        self.stack_ptr = 0;
        self.stack = [0; STACK_SIZE];
        self.keys = [false; NUM_KEYS];
        self.delay_t = 0;
        self.sound_t = 0;
        self.ram[..FONTSET_SIZE].copy_from_slice(&FONTSET);
    }

    // CPU operations
    pub fn tick(&mut self) {
        // basic tick process
        //1. fetch value from the game that has already been loaded into ram at the program counter
        //2. decode the instruction
        //3. execute, may involve editing the registers or stack
        //4. move pc to the next instruction, repeat
        let opcode = self.fetch();
        self.execute(opcode);
    }

    fn fetch(&mut self) -> u16 {
        // chip8 opcodes are exactly 2 bytes and store the information needed inside them instead of elsewhere
        let higher_byte = self.ram[self.pc as usize] as u16; // fetch 1 byte
        let lower_byte = self.ram[(self.pc + 1) as usize] as u16; // fetch other byte for 16 total bits

        // logical shift higher byte left 8, then OR lower byte into the freed 8 bits
        let op = (higher_byte << 8) | lower_byte; // combine both bytes into 8 bit value by big endian
        self.pc += 2; // increase past these most recent ram addressses
        op
    }

    fn execute(&mut self, op: u16) {
        let d1 = (op & 0xF000) >> 12;
        let d2 = (op & 0x0F00) >> 8;
        let d3 = (op & 0x00F0) >> 4;
        let d4 = op & 0x000F;

        match (d1, d2, d3, d4) {
            // NOP
            (0,0,0,0) => return,
            // CLS
            (0,0,0xE,0) => {
                self.display = [false; SCREEN_WIDTH * SCREEN_HEIGHT];
            },
            // RET
            (0,0,0xE,0xE) => { // when entering subroutine, push current address onto stack, this function then pops it back when returning
                let return_addr = self.pop();
                self.pc = return_addr;
            },
            // JMP 0xNNN
            (1,_,_,_) => { // set pc to the given op code address
                let nnn = op & 0xFFF;
                self.pc = nnn;
            },
            // CALL 0xNNN
            (2,_,_,_) => { // set pc to the given op code address, _ wildcard, catch all op start with 2
                let nnn = op & 0xFFF;
                self.push(self.pc);
                self.pc = nnn;
            },
            // SKIP VX == NN
            (3,_,_,_) => {
                let x = d2 as usize;
                let nn = (op & 0xFF) as u8;
                if self.v_registers[x] == nn {
                    self.pc += 2; // skip to next opcode, aka increase pc by 2 bytes
                }
            },
            // SKIP VX == NN
            (4,_,_,_) => {
                let x = d2 as usize;
                let y = d3 as usize;
                if self.v_registers[x] == self.v_registers[y] {
                    self.pc += 2; // skip to next opcode, aka increase pc by 2 bytes
                }
            },
            // SKIP VX == NN
            (5,_,_,0) => {
                let x = d2 as usize;
                let nn = (op & 0xFF) as u8; // & 0xFF gives last 8 bits of op
                if self.v_registers[x] != nn {
                    self.pc += 2; // skip to next opcode, aka increase pc by 2 bytes
                }
            },
            // VX = NN
            (6,_,_,_) => {
                let x = d2 as usize;
                let nn = (op & 0xFF) as u8; // & 0xFF gives last 8 bits of op
                self.v_registers[x] = nn;
            },
            // VX += NN
            (7,_,_,_) => {
                let x = d2 as usize;
                let nn = (op & 0xFF) as u8; // & 0xFF gives last 8 bits of op
                self.v_registers[x] = self.v_registers[x].wrapping_add(nn); // rust will panic on overflow
            },
            // VX = VY
            (8,_,_,0) => {
                let x = d2 as usize;
                let y = d3 as usize;
                self.v_registers[x] = self.v_registers[y];
            },
            // VX |= VY (OR bitwise)
            (8,_,_,1) => {
                let x = d2 as usize;
                let y = d3 as usize;
                self.v_registers[x] |= self.v_registers[y];
            },
            // VX &= VY (AND bitwise)
            (8,_,_,2) => {
                let x = d2 as usize;
                let y = d3 as usize;
                self.v_registers[x] &= self.v_registers[y];
            },
            // VX ^= VY (XOR bitwise)
            (8,_,_,3) => {
                let x = d2 as usize;
                let y = d3 as usize;
                self.v_registers[x] ^= self.v_registers[y];
            },
            // VX += VY
            (8,_,_,4) => {
                let x = d2 as usize;
                let y = d3 as usize;
                let (new_vx, carry) = self.v_registers[x].overflowing_add(self.v_registers[y]);
                let new_vf = if carry { 1 } else { 0 };
                self.v_registers[x] = new_vx;
                self.v_registers[0xF] = new_vf; // last register is the flag register that is a bool which denotes if the last operation resulted in an over/underflow
            },
            // VX -= VY
            (8,_,_,5) => {
                let x = d2 as usize;
                let y = d3 as usize;
                let (new_vx, borrow) = self.v_registers[x].overflowing_sub(self.v_registers[y]);
                let new_vf = if borrow { 0 } else { 1 }; // convert bool to int since the registers are all ints

                self.v_registers[x] = new_vx;
                self.v_registers[0xF] = new_vf; // last register is the flag register that is a bool which denotes if the last operation resulted in an over/underflow
            },
            // VX >> 1
            (8,_,_,6) => {
                let x = d2 as usize;
                let lsb = self.v_registers[x] & 1; //least significant bit, catch and set VF
                self.v_registers[x] >>= 1; // right shift equal
                self.v_registers[0xF] = lsb;
            },
            // VX = VY - VX
            (8,_,_,7) => {
                let x = d2 as usize;
                let y = d3 as usize;
                let (new_vx, borrow) = self.v_registers[y].overflowing_sub(self.v_registers[x]);
                let new_vf = if borrow { 0 } else { 1 };
                self.v_registers[x] = new_vx;
                self.v_registers[0xF] = new_vf;
            },
            // VX << 1
            (8,_,_,0xE) => {
                let x = d2 as usize;
                let msb = (self.v_registers[x] >> 7) & 1; //most significant bit, catch and set VF
                self.v_registers[x] <<= 1; // right shift equal
                self.v_registers[0xF] = msb;
            },
            // SKIP VX != VY
            (9,_,_,0) => {
                let x = d2 as usize;
                let y = d3 as usize;
                if self.v_registers[x] != self.v_registers[y] {
                    self.pc += 2;
                }
            },
            // SET I register
            (0xA,_,_,_) => {
                let nnn = op & 0xFFF;
                self.i_register = nnn;
            },
            // SET pc to I register 0 value plus input
            (0xB,_,_,_) => {
                let nnn = op & 0xFFF;
                self.pc = (self.v_registers[0] as u16) + nnn;
            },
            // VX = rand() & NN
            (0xC,_,_,_) => {
                let x = d2 as usize;
                let nn = (op & 0xFFF) as u8;
                let rng:u8 = random();
                self.v_registers[x] = rng & nn;
            },
            // Draw Sprite XY
            (0xD,_,_,_) => {
                let x_cord = self.v_registers[d2 as usize] as u16;
                let y_cord = self.v_registers[d3 as usize] as u16;
                let num_rows = d4;
                // chip 8 sprites are always 8 pixels wide, variable pixels tall (specified in d4)

                let mut flipped = false; // keep track if any pixels were flipped (black <-> white)
                // iterate over each row of the sprite
                for y_line in 0..num_rows {
                    let addr = self.i_register + y_line as u16;
                    let pixels = self.ram[addr as usize];
                    // iterate over each column in the row
                    for x_line in 0..8 {
                        // fetch current pixels bit
                        if (pixels & (0b1000_0000 >> x_line)) != 0 { // only flip if a one
                            // wrap sprites around screen
                            let x = (x_cord + x_line) as usize % SCREEN_WIDTH;
                            let y = (y_cord + y_line) as usize % SCREEN_HEIGHT;
                            
                            // get pixels idx over the 1d screen array
                            let idx = x + SCREEN_WIDTH * y;
                            flipped |= self.display[idx];
                            self.display[idx] ^= true;
                        }
                    }
                }
                if flipped {
                    self.v_registers[0] = 1;
                }
                else {
                    self.v_registers[0] = 0;
                }
            },
            // SKIP KEY PRESS
            (0xE,_,9,0xE) => {
                let x = d2 as usize;
                let vx = self.v_registers[x];
                let key = self.keys[vx as usize];
                if key {
                    self.pc += 2;
                }
            },
            // SKIP KEY RELEASE
            (0xE,_,0xA,1) => {
                let x = d2 as usize;
                let vx = self.v_registers[x];
                let key = self.keys[vx as usize];
                if !key {
                    self.pc += 2;
                }
            },
            // VX = DT
            (0xF,_,0,7) => {
                let x = d2 as usize;
                self.v_registers[x] = self.delay_t;
            },
            // WAIT KEY PRESS
            (0xF,_,0,0xA) => {
                let x = d2 as usize;
                let mut pressed = false;
                for i in 0..self.keys.len() {
                    if self.keys[i] {
                        self.v_registers[x] = i as u8;
                        pressed = true;
                        break;
                    }
                }
                if !pressed { // redo opcode
                    self.pc -= 2;
                }
            },
            // DT = VX
            (0xF,_,1,5) => {
                let x = d2 as usize;
                self.delay_t = self.v_registers[x];
            },
            // ST = VX
            (0xF,_,1,8) => {
                let x = d2 as usize;
                self.sound_t = self.v_registers[x];
            },
            // I += VX
            (0xF,_,1,0xE) => {
                let x = d2 as usize;
                let vx = self.v_registers[x] as u16;
                self.i_register += self.i_register.wrapping_add(vx);
            },
            // I = FONT
            (0xF,_,2,9) => {
                let x = d2 as usize;
                let c = self.v_registers[x] as u16;
                self.i_register = c * 5;
            },
            // BCD
            (0xF,_,3,3) => {
                let x = d2 as usize;
                let vx = self.v_registers[x] as f32;

                let hundreds = (vx / 100.0).floor() as u8;
                let tens = ((vx / 10.0) % 10.0).floor() as u8;
                let ones = (vx % 10.0) as u8;

                self.ram[self.i_register as usize] = hundreds;
                self.ram[(self.i_register + 1) as usize] = tens;
                self.ram[(self.i_register + 2) as usize] = ones;
            },
            // STORE V0-VX
            (0xF,_,5,5) => {
                let x = d2 as usize;
                let i = self.i_register as usize;
                for idx in 0..=x {
                    self.ram[i+idx] = self.v_registers[idx];
                }
            },
            // LOAD V0-VX
            (0xF,_,6,5) => {
                let x = d2 as usize;
                let i = self.i_register as usize;
                for idx in 0..=x {
                    self.v_registers[idx] = self.ram[i + idx];
                }
            },
            (_, _, _, _) => unimplemented!("Unimplemented opcode: {}", op) // catch all
        }
    }

    pub fn tick_timers(&mut self) {
        if self.delay_t > 0 {
            self.delay_t -= 1; // count down
        }

        if self.sound_t > 0 {
            if self.sound_t == 0 {
                //beep 
            }
            self.sound_t -= 1; // count down
        }
    }

    // stack operations
    pub fn push(&mut self, val:u16) {
        self.stack[self.stack_ptr as usize] = val; // change the current stack pointer to the val
        self.stack_ptr += 1 // increase the stack pointer after modification
    }

    pub fn pop(&mut self) -> u16 {
        self.stack_ptr -= 1; // move to previous position
        self.stack[self.stack_ptr as usize] // return the value of the stack at the pointer
    }

    // interaction operations
    pub fn get_display(&self) -> &[bool] {
        &self.display
    }

    pub fn keypress(&mut self, idx:usize, pressed:bool) {
        self.keys[idx] = pressed;
    }

    // ram operations
    pub fn load(&mut self, data: &[u8]) {
        let start = START_ADDR as usize;
        let end = (START_ADDR as usize) + data.len();
        self.ram[start..end].copy_from_slice(data);
    }
}