-- nil key
local t = {}
local ok, err = pcall(function ()
  t[nil] = 'this should fails'
end)

assert(not ok)
assert(err:sub(-24) == 'badkey.lua:4: key is nil')

-- NaN key
local ok, err = pcall(function ()
  t[0/0] = 'this should fails'
end)

assert(not ok)
assert(err:sub(-25) == 'badkey.lua:12: key is NaN')
