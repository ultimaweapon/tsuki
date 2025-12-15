-- nil key
local t = {}
local ok, msg, src, ln = pcall(function ()
  t[nil] = 'this should fails'
end)

assert(not ok)
assert(msg == 'key is nil')
assert(src:sub(-10) == 'badkey.lua')
assert(ln == 4)

-- NaN key
local ok, msg = pcall(function ()
  t[0/0] = 'this should fails'
end)

assert(not ok)
assert(msg == 'key is NaN')
