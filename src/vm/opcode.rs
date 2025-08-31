#![allow(
    dead_code,
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

pub type OpCode = libc::c_uint;
pub type OpMode = libc::c_uint;

pub const OP_EXTRAARG: OpCode = 82;
pub const OP_VARARGPREP: OpCode = 81;
pub const OP_VARARG: OpCode = 80;
pub const OP_CLOSURE: OpCode = 79;
pub const OP_SETLIST: OpCode = 78;
pub const OP_TFORLOOP: OpCode = 77;
pub const OP_TFORCALL: OpCode = 76;
pub const OP_TFORPREP: OpCode = 75;
pub const OP_FORPREP: OpCode = 74;
pub const OP_FORLOOP: OpCode = 73;
pub const OP_RETURN1: OpCode = 72;
pub const OP_RETURN0: OpCode = 71;
pub const OP_RETURN: OpCode = 70;
pub const OP_TAILCALL: OpCode = 69;
pub const OP_CALL: OpCode = 68;
pub const OP_TESTSET: OpCode = 67;
pub const OP_TEST: OpCode = 66;
pub const OP_GEI: OpCode = 65;
pub const OP_GTI: OpCode = 64;
pub const OP_LEI: OpCode = 63;
pub const OP_LTI: OpCode = 62;
pub const OP_EQI: OpCode = 61;
pub const OP_EQK: OpCode = 60;
pub const OP_LE: OpCode = 59;
pub const OP_LT: OpCode = 58;
pub const OP_EQ: OpCode = 57;
pub const OP_JMP: OpCode = 56;
pub const OP_TBC: OpCode = 55;
pub const OP_CLOSE: OpCode = 54;
pub const OP_CONCAT: OpCode = 53;
pub const OP_LEN: OpCode = 52;
pub const OP_NOT: OpCode = 51;
pub const OP_BNOT: OpCode = 50;
pub const OP_UNM: OpCode = 49;
pub const OP_MMBINK: OpCode = 48;
pub const OP_MMBINI: OpCode = 47;
pub const OP_MMBIN: OpCode = 46;
pub const OP_SHR: OpCode = 45;
pub const OP_SHL: OpCode = 44;
pub const OP_BXOR: OpCode = 43;
pub const OP_BOR: OpCode = 42;
pub const OP_BAND: OpCode = 41;
pub const OP_IDIV: OpCode = 40;
pub const OP_DIV: OpCode = 39;
pub const OP_POW: OpCode = 38;
pub const OP_MOD: OpCode = 37;
pub const OP_MUL: OpCode = 36;
pub const OP_SUB: OpCode = 35;
pub const OP_ADD: OpCode = 34;
pub const OP_SHLI: OpCode = 33;
pub const OP_SHRI: OpCode = 32;
pub const OP_BXORK: OpCode = 31;
pub const OP_BORK: OpCode = 30;
pub const OP_BANDK: OpCode = 29;
pub const OP_IDIVK: OpCode = 28;
pub const OP_DIVK: OpCode = 27;
pub const OP_POWK: OpCode = 26;
pub const OP_MODK: OpCode = 25;
pub const OP_MULK: OpCode = 24;
pub const OP_SUBK: OpCode = 23;
pub const OP_ADDK: OpCode = 22;
pub const OP_ADDI: OpCode = 21;
pub const OP_SELF: OpCode = 20;
pub const OP_NEWTABLE: OpCode = 19;
pub const OP_SETFIELD: OpCode = 18;
pub const OP_SETI: OpCode = 17;
pub const OP_SETTABLE: OpCode = 16;
pub const OP_SETTABUP: OpCode = 15;
pub const OP_GETFIELD: OpCode = 14;
pub const OP_GETI: OpCode = 13;
pub const OP_GETTABLE: OpCode = 12;
pub const OP_GETTABUP: OpCode = 11;
pub const OP_SETUPVAL: OpCode = 10;
pub const OP_GETUPVAL: OpCode = 9;
pub const OP_LOADNIL: OpCode = 8;
pub const OP_LOADTRUE: OpCode = 7;
pub const OP_LFALSESKIP: OpCode = 6;
pub const OP_LOADFALSE: OpCode = 5;
pub const OP_LOADKX: OpCode = 4;
pub const OP_LOADK: OpCode = 3;
pub const OP_LOADF: OpCode = 2;
pub const OP_LOADI: OpCode = 1;
pub const OP_MOVE: OpCode = 0;

pub const isJ: OpMode = 4;
pub const iAx: OpMode = 3;
pub const iAsBx: OpMode = 2;
pub const iABx: OpMode = 1;
pub const iABC: OpMode = 0;

pub static mut luaP_opmodes: [u8; 83] = [
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iAsBx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iAsBx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((1 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((1 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((1 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | isJ as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (1 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int
        | (1 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int
        | (1 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (1 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (1 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABx as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (1 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 3 as libc::c_int
        | iABC as libc::c_int) as u8,
    ((0 as libc::c_int) << 7 as libc::c_int
        | (0 as libc::c_int) << 6 as libc::c_int
        | (0 as libc::c_int) << 5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (0 as libc::c_int) << 3 as libc::c_int
        | iAx as libc::c_int) as u8,
];
