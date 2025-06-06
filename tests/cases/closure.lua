print "testing closures"

do  -- bug in 5.4.7
  _ENV[true] = 10
  local function aux () return _ENV[1 < 2] end
  assert(aux() == 10)
  _ENV[true] = nil
end

-- testing equality
a = {}

for i = 1, 5 do  a[i] = function (x) return i + a + _ENV end  end
assert(a[3] ~= a[4] and a[4] ~= a[5])

do
  local a = function (x)  return math.sin(_ENV[x])  end
  local function f()
    return a
  end
  assert(f() == f())
end


-- testing closures with 'for' control variable
a = {}
for i=1,10 do
  a[i] = {set = function(x) i=x end, get = function () return i end}
  if i == 3 then break end
end
assert(a[4] == undef)
a[1].set(10)
assert(a[2].get() == 2)
a[2].set('a')
assert(a[3].get() == 3)
assert(a[2].get() == 'a')

a = {}
local t = {"a", "b"}
for i = 1, #t do
  local k = t[i]
  a[i] = {set = function(x, y) i=x; k=y end,
          get = function () return i, k end}
  if i == 2 then break end
end
a[1].set(10, 20)
local r,s = a[2].get()
assert(r == 2 and s == 'b')
r,s = a[1].get()
assert(r == 10 and s == 20)
a[2].set('a', 'b')
r,s = a[2].get()
assert(r == "a" and s == "b")


-- testing closures with 'for' control variable x break
local f
for i=1,3 do
  f = function () return i end
  break
end
assert(f() == 1)

for k = 1, #t do
  local v = t[k]
  f = function () return k, v end
  break
end
assert(({f()})[1] == 1)
assert(({f()})[2] == "a")


-- testing closure x break x return x errors

local b
function f(x)
  local first = 1
  while 1 do
    if x == 3 and not first then return end
    local a = 'xuxu'
    b = function (op, y)
          if op == 'set' then
            a = x+y
          else
            return a
          end
        end
    if x == 1 then do break end
    elseif x == 2 then return
    else if x ~= 3 then error() end
    end
    first = nil
  end
end

for i=1,3 do
  f(i)
  assert(b('get') == 'xuxu')
  b('set', 10); assert(b('get') == 10+i)
  b = nil
end

pcall(f, 4);
assert(b('get') == 'xuxu')
b('set', 10); assert(b('get') == 14)


local y, w
-- testing multi-level closure
function f(x)
  return function (y)
    return function (z) return w+x+y+z end
  end
end

y = f(10)
w = 1.345
assert(y(20)(30) == 60+w)


-- testing closures x break
do
  local X, Y
  local a = math.sin(0)

  while a do
    local b = 10
    X = function () return b end   -- closure with upvalue
    if a then break end
  end

  do
    local b = 20
    Y = function () return b end   -- closure with upvalue
  end

  -- upvalues must be different
  assert(X() == 10 and Y() == 20)
end


-- testing closures x repeat-until

local a = {}
local i = 1
repeat
  local x = i
  a[i] = function () i = x+1; return x end
until i > 10 or a[i]() ~= x
assert(i == 11 and a[1]() == 1 and a[3]() == 3 and i == 4)


-- testing closures created in 'then' and 'else' parts of 'if's
a = {}
for i = 1, 10 do
  if i % 3 == 0 then
    local y = 0
    a[i] = function (x) local t = y; y = x; return t end
  elseif i % 3 == 1 then
    goto L1
    error'not here'
  ::L1::
    local y = 1
    a[i] = function (x) local t = y; y = x; return t end
  elseif i % 3 == 2 then
    local t
    goto l4
    ::l4a:: a[i] = t; goto l4b
    error("should never be here!")
    ::l4::
    local y = 2
    t = function (x) local t = y; y = x; return t end
    goto l4a
    error("should never be here!")
    ::l4b::
  end
end

for i = 1, 10 do
  assert(a[i](i * 10) == i % 3 and a[i]() == i * 10)
end

print'+'


-- test for correctly closing upvalues in tail calls of vararg functions
local function t ()
  local function c(a,b) assert(a=="test" and b=="OK") end
  local function v(f, ...) c("test", f() ~= 1 and "FAILED" or "OK") end
  local x = 1
  return v(function() return x end)
end
t()

print'OK'
