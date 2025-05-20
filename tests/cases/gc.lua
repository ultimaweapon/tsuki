print('testing incremental garbage collection')

assert(collectgarbage("isrunning"))

collectgarbage()

local oldmode = collectgarbage("incremental")

-- changing modes should return previous mode
assert(collectgarbage("generational") == "incremental")
assert(collectgarbage("generational") == "generational")
assert(collectgarbage("incremental") == "generational")
assert(collectgarbage("incremental") == "incremental")


local function nop () end

local function gcinfo ()
  return collectgarbage"count" * 1024
end


-- test weird parameters to 'collectgarbage'
do
  -- save original parameters
  local a = collectgarbage("setpause", 200)
  local b = collectgarbage("setstepmul", 200)
  local t = {0, 2, 10, 90, 500, 5000, 30000, 0x7ffffffe}
  for i = 1, #t do
    local p = t[i]
    for j = 1, #t do
      local m = t[j]
      collectgarbage("setpause", p)
      collectgarbage("setstepmul", m)
      collectgarbage("step", 0)
      collectgarbage("step", 10000)
    end
  end
  -- restore original parameters
  collectgarbage("setpause", a)
  collectgarbage("setstepmul", b)
  collectgarbage()
end


_G["while"] = 234

do
  print("creating many objects")

  local limit = 5000

  for i = 1, limit do
    local a = {}; a = nil
  end

  local a = "a"

  for i = 1, limit do
    a = i .. "b";
    a = string.gsub(a, '(%d%d*)', "%1 %1")
    a = "a"
  end



  a = {}

  function a:test ()
    for i = 1, limit do
      load(string.format("function temp(a) return 'a%d' end", i), "")()
      assert(temp() == string.format('a%d', i))
    end
  end

  a:test()
  _G.temp = nil
end


-- collection of functions without locals, globals, etc.
do local f = function () end end


print("functions with errors")
local prog = [[
do
  a = 10;
  function foo(x,y)
    a = sin(a+0.456-0.23e-12);
    return function (z) return sin(%x+z) end
  end
  local x = function (w) a=a+w; end
end
]]
do
  local step = 1
  if _soft then step = 13 end
  for i=1, string.len(prog), step do
    for j=i, string.len(prog), step do
      pcall(load(string.sub(prog, i, j), ""))
    end
  end
end
rawset(_G, "a", nil)
_G.x = nil

do
  foo = nil
  print('long strings')
  local x = "01234567890123456789012345678901234567890123456789012345678901234567890123456789"
  assert(string.len(x)==80)
  local s = ''
  local k = math.min(300, (math.maxinteger // 80) // 2)
  for n = 1, k do s = s..x; local j=tostring(n)  end
  assert(string.len(s) == k*80)
  s = string.sub(s, 1, 10000)
  local s, i = string.gsub(s, '(%d%d%d%d)', '')
  assert(i==10000 // 4)

  assert(_G["while"] == 234)
  _G["while"] = nil
end


--
-- test the "size" of basic GC steps (whatever they mean...)
--
do
print("steps")

  print("steps (2)")

  local function dosteps (siz)
    collectgarbage()
    local a = {}
    for i=1,100 do a[i] = {{}}; local b = {} end
    local x = gcinfo()
    local i = 0
    repeat   -- do steps until it completes a collection cycle
      i = i+1
    until collectgarbage("step", siz)
    assert(gcinfo() < x)
    return i    -- number of steps
  end

  collectgarbage"stop"

  if not _port then
    assert(dosteps(10) < dosteps(2))
  end

  -- collector should do a full collection with so many steps
  assert(dosteps(20000) == 1)
  assert(collectgarbage("step", 20000) == true)
  assert(collectgarbage("step", 20000) == true)

  assert(not collectgarbage("isrunning"))
  collectgarbage"restart"
  assert(collectgarbage("isrunning"))

end


if not _port then
  -- test the pace of the collector
  collectgarbage(); collectgarbage()
  local x = gcinfo()
  collectgarbage"stop"
  repeat
    local a = {}
  until gcinfo() > 3 * x
  collectgarbage"restart"
  assert(collectgarbage("isrunning"))
  repeat
    local a = {}
  until gcinfo() <= x * 2
end


print("clearing tables")
local lim = 15
local a = {}
-- fill a with `collectable' indices
for i=1,lim do a[{}] = i end
b = {}
for k,v in pairs(a) do b[k]=v end
-- remove all indices and collect them
for n in pairs(b) do
  a[n] = undef
  assert(type(n) == 'table' and next(n) == nil)
  collectgarbage()
end
b = nil
collectgarbage()
for n in pairs(a) do error'cannot be here' end
for i=1,lim do a[i] = i end
for i=1,lim do assert(a[i] == i) end


print('weak tables')
a = {}; setmetatable(a, {__mode = 'k'});
-- fill a with some `collectable' indices
for i=1,lim do a[{}] = i end
-- and some non-collectable ones
for i=1,lim do a[i] = i end
for i=1,lim do local s=string.rep('@', i); a[s] = s..'#' end
collectgarbage()
local i = 0
for k,v in pairs(a) do assert(k==v or k..'#'==v); i=i+1 end
assert(i == 2*lim)

a = {}; setmetatable(a, {__mode = 'v'});
a[1] = string.rep('b', 21)
collectgarbage()
assert(a[1])   -- strings are *values*
a[1] = undef
-- fill a with some `collectable' values (in both parts of the table)
for i=1,lim do a[i] = {} end
for i=1,lim do a[i..'x'] = {} end
-- and some non-collectable ones
for i=1,lim do local t={}; a[t]=t end
for i=1,lim do a[i+lim]=i..'x' end
collectgarbage()
local i = 0
for k,v in pairs(a) do assert(k==v or k-lim..'x' == v); i=i+1 end
assert(i == 2*lim)

a = {}; setmetatable(a, {__mode = 'kv'});
local x, y, z = {}, {}, {}
-- keep only some items
a[1], a[2], a[3] = x, y, z
a[string.rep('$', 11)] = string.rep('$', 11)
-- fill a with some `collectable' values
for i=4,lim do a[i] = {} end
for i=1,lim do a[{}] = i end
for i=1,lim do local t={}; a[t]=t end
collectgarbage()
assert(next(a) ~= nil)
local i = 0
for k,v in pairs(a) do
  assert((k == 1 and v == x) or
         (k == 2 and v == y) or
         (k == 3 and v == z) or k==v);
  i = i+1
end
assert(i == 4)
x,y,z=nil
collectgarbage()
assert(next(a) == string.rep('$', 11))


-- 'bug' in 5.1
a = {}
local t = {x = 10}
local C = setmetatable({key = t}, {__mode = 'v'})
local C1 = setmetatable({[t] = 1}, {__mode = 'k'})
a.x = t  -- this should not prevent 't' from being removed from
         -- weak table 'C' by the time 'a' is finalized

a, t = nil
collectgarbage()
collectgarbage()
assert(next(C) == nil and next(C1) == nil)
C, C1 = nil

-- __gc x weak tables
local u = setmetatable({}, {__gc = true})
-- __gc metamethod should be collected before running
setmetatable(getmetatable(u), {__mode = "v"})
u = nil
collectgarbage()

do   -- tests for string keys in weak tables
  collectgarbage(); collectgarbage()
  local m = collectgarbage("count")         -- current memory
  local a = setmetatable({}, {__mode = "kv"})
  a[string.rep("a", 2^22)] = 25   -- long string key -> number value
  a[string.rep("b", 2^22)] = {}   -- long string key -> colectable value
  a[{}] = 14                     -- colectable key
  assert(collectgarbage("count") > m + 2^13)    -- 2^13 == 2 * 2^22 in KB
  collectgarbage()
  assert(collectgarbage("count") >= m + 2^12 and
        collectgarbage("count") < m + 2^13)    -- one key was collected
  local k, v = next(a)   -- string key with number value preserved
  assert(k == string.rep("a", 2^22) and v == 25)
  assert(next(a, k) == nil)  -- everything else cleared
  assert(a[string.rep("b", 2^22)] == undef)
  a[k] = undef        -- erase this last entry
  k = nil
  collectgarbage()
  assert(next(a) == nil)
  -- make sure will not try to compare with dead key
  assert(a[string.rep("b", 100)] == undef)
  assert(collectgarbage("count") <= m + 1)   -- eveything collected
end

if not _soft then
  print("long list")
  local a = {}
  for i = 1,200000 do
    a = {next = a}
  end
  a = nil
  collectgarbage()
end

do
  collectgarbage()
  collectgarbage"stop"
  collectgarbage("step", 0)   -- steps should not unblock the collector
  local x = gcinfo()
  repeat
    for i=1,1000 do _ENV.a = {} end   -- no collection during the loop
  until gcinfo() > 2 * x
  collectgarbage"restart"
  _ENV.a = nil
end

-- create an object to be collected when state is closed
do
  local setmetatable,assert,type,print,getmetatable =
        setmetatable,assert,type,print,getmetatable
  local tt = {}
  local u = setmetatable({}, tt)
  ___Glob = {u}   -- avoid object being collected before program end
end

-- just to make sure
assert(collectgarbage'isrunning')


collectgarbage(oldmode)

print('OK')
