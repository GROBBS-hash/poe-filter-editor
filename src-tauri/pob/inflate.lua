-- Inflate / Deflate for Path of Building without its window: its own zlib1.dll through LuaJIT's FFI (the window
-- version gets these from SimpleGraphic; the headless stubs return "", so timeless jewel data didn't load).
local ffi = require("ffi")
ffi.cdef[[
typedef struct z_stream_s {
	const unsigned char *next_in; unsigned int avail_in; unsigned long total_in;
	unsigned char *next_out; unsigned int avail_out; unsigned long total_out;
	const char *msg; void *state; void *zalloc; void *zfree; void *opaque;
	int data_type; unsigned long adler; unsigned long reserved;
} z_stream;
const char *zlibVersion(void);
int inflateInit2_(z_stream *strm, int windowBits, const char *version, int stream_size);
int inflate(z_stream *strm, int flush);
int inflateEnd(z_stream *strm);
int deflateInit2_(z_stream *strm, int level, int method, int windowBits, int memLevel, int strategy, const char *version, int stream_size);
int deflate(z_stream *strm, int flush);
int deflateEnd(z_stream *strm);
]]
local z = ffi.load("zlib1")
local CHUNK = 262144

local function run(init, step, finish, data, flushAll)
	local s = ffi.new("z_stream")
	if init(s) ~= 0 then return nil, "zlib init failed" end
	local input = ffi.new("unsigned char[?]", #data)
	ffi.copy(input, data, #data)
	s.next_in, s.avail_in = input, #data
	local out, buf = {}, ffi.new("unsigned char[?]", CHUNK)
	repeat
		s.next_out, s.avail_out = buf, CHUNK
		local r = step(s, flushAll)
		if r ~= 0 and r ~= 1 then finish(s); return nil, "zlib error " .. r end
		out[#out + 1] = ffi.string(buf, CHUNK - s.avail_out)
	until r == 1 or (s.avail_out ~= 0 and s.avail_in == 0)
	finish(s)
	return table.concat(out)
end

function Inflate(data)
	-- 15 + 32: zlib or gzip header, found by itself.
	return run(function(s) return z.inflateInit2_(s, 15 + 32, z.zlibVersion(), ffi.sizeof("z_stream")) end, z.inflate, z.inflateEnd, data, 0)
end

function Deflate(data, isGzip)
	return run(function(s) return z.deflateInit2_(s, 9, 8, isGzip and 31 or 15, 8, 0, z.zlibVersion(), ffi.sizeof("z_stream")) end, z.deflate, z.deflateEnd, data, 4)
end
