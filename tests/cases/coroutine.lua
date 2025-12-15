print "testing coroutines"

local f

local main, ismain = coroutine.running()
assert(type(main) == "thread" and not ismain)
assert(not coroutine.resume(main))
assert(not coroutine.isyieldable())
assert(not pcall(coroutine.yield))


-- trivial errors
assert(not pcall(coroutine.resume, 0))
assert(not pcall(coroutine.status, 0))


-- tests for multiple yield/resume arguments

local function eqtab (t1, t2)
  assert(#t1 == #t2)
  for i = 1, #t1 do
    local v = t1[i]
    assert(t2[i] == v)
  end
end

_G.x = nil   -- declare x
_G.f = nil   -- declare f
local function foo (a, ...)
  local x, y = coroutine.running()
  assert(x == f and y == false)
  -- next call should not corrupt coroutine (but must fail,
  -- as it attempts to resume the running coroutine)
  assert(coroutine.resume(f) == false)
  assert(coroutine.status(f) == "running")
  local arg = {...}
  assert(coroutine.isyieldable(x))
  for i=1,#arg do
    _G.x = {coroutine.yield(table.unpack(arg[i]))}
  end
  return table.unpack(a)
end

f = coroutine.create(foo)
assert(coroutine.isyieldable(f))
assert(type(f) == "thread" and coroutine.status(f) == "suspended")
assert(string.find(tostring(f), "thread"))
local s,a,b,c,d
s,a,b,c,d = coroutine.resume(f, {1,2,3}, {}, {1}, {'a', 'b', 'c'})
assert(coroutine.isyieldable(f))
assert(s and a == nil and coroutine.status(f) == "suspended")
s,a,b,c,d = coroutine.resume(f)
eqtab(_G.x, {})
assert(s and a == 1 and b == nil)
assert(coroutine.isyieldable(f))
s,a,b,c,d = coroutine.resume(f, 1, 2, 3)
eqtab(_G.x, {1, 2, 3})
assert(s and a == 'a' and b == 'b' and c == 'c' and d == nil)
s,a,b,c,d = coroutine.resume(f, "xuxu")
eqtab(_G.x, {"xuxu"})
assert(s and a == 1 and b == 2 and c == 3 and d == nil)
assert(coroutine.status(f) == "dead")
s, a = coroutine.resume(f, "xuxu")
assert(not s and string.find(a, "dead") and coroutine.status(f) == "dead")

_G.f = nil

-- yields in tail calls
local function foo (i) return coroutine.yield(i) end
local f = coroutine.wrap(function ()
  for i=1,10 do
    assert(foo(i) == _G.x)
  end
  return 'a'
end)
for i=1,10 do _G.x = i; assert(f(i) == i) end
_G.x = 'xuxu'; assert(f('xuxu') == 'a')

_G.x = nil

-- recursive
local function pf (n, i)
  coroutine.yield(n)
  pf(n*i, i+1)
end

f = coroutine.wrap(pf)
local s=1
for i=1,10 do
  assert(f(1, 1) == s)
  s = s*i
end

-- sieve
local function gen (n)
  return coroutine.wrap(function ()
    for i=2,n do coroutine.yield(i) end
  end)
end


local function filter (p, g)
  return coroutine.wrap(function ()
    while 1 do
      local n = g()
      if n == nil then return end
      if math.fmod(n, p) ~= 0 then coroutine.yield(n) end
    end
  end)
end

local x = gen(80)
local a = {}
while 1 do
  local n = x()
  if n == nil then break end
  table.insert(a, n)
  x = filter(n, x)
end

assert(#a == 22 and a[#a] == 79)
x, a = nil


print("to-be-closed variables in coroutines")

local function func2close (f)
  return setmetatable({}, {__close = f})
end

do
  -- ok to close a dead coroutine
  local co = coroutine.create(print)
  assert(coroutine.resume(co, "testing 'coroutine.close'"))
  assert(coroutine.status(co) == "dead")
  local st, msg = coroutine.close(co)
  assert(st and msg == nil)
  -- also ok to close it again
  st, msg = coroutine.close(co)
  assert(st and msg == nil)


  -- cannot close the running coroutine
  local st, msg = pcall(coroutine.close, coroutine.running())
  assert(not st and string.find(msg, "running"))

  local main = coroutine.running()

  -- cannot close a "normal" coroutine
  ;(coroutine.wrap(function ()
    local st, msg = pcall(coroutine.close, main)
    assert(not st and string.find(msg, "normal"))
  end))()

  -- cannot close a coroutine while closing it
  do
    local co
    co = coroutine.create(
      function()
        local x <close> = func2close(function()
            coroutine.close(co)   -- try to close it again
         end)
        coroutine.yield(20)
      end)
    local st, msg = coroutine.resume(co)
    assert(st and msg == 20)
    st, msg = coroutine.close(co)
    assert(not st and string.find(msg, "running coroutine"))
  end

  -- to-be-closed variables in coroutines
  local X

  -- closing a coroutine after an error
  local co = coroutine.create(error)
  local st, msg = coroutine.resume(co, 100)
  assert(not st and msg == 100)
  st, msg = coroutine.close(co)
  assert(not st and msg == 100)
  -- after closing, no more errors
  st, msg = coroutine.close(co)
  assert(st and msg == nil)

  co = coroutine.create(function ()
    local x <close> = func2close(function (self, err)
      assert(err == nil); X = false
    end)
    X = true
    coroutine.yield()
  end)
  coroutine.resume(co)
  assert(X)
  assert(coroutine.close(co))
  assert(not X and coroutine.status(co) == "dead")

  -- error closing a coroutine
  local x = 0
  co = coroutine.create(function()
    local y <close> = func2close(function (self,err)
      assert(err == 111)
      x = 200
      error(200)
    end)
    local x <close> = func2close(function (self, err)
      assert(err == nil); error(111)
    end)
    coroutine.yield()
  end)
  coroutine.resume(co)
  assert(x == 0)
  local st, msg = coroutine.close(co)
  assert(st == false and coroutine.status(co) == "dead" and msg == 200)
  assert(x == 200)
  -- after closing, no more errors
  st, msg = coroutine.close(co)
  assert(st and msg == nil)
end

do
  -- <close> versus pcall in coroutines
  local X = false
  local function foo ()
    local x <close> = func2close(function (self, err)
      X = err
    end)
    error(43)
  end
  local co = coroutine.create(function () return pcall(foo) end)
  local st1, st2, err = coroutine.resume(co)
  assert(st1 and not st2 and err == 43)
  assert(X == 43)

  -- recovering from errors in __close metamethods
  local track = {}

  local function h (o)
    local hv <close> = o
    return 1
  end

  local function foo ()
    local x <close> = func2close(function(_,msg)
      track[#track + 1] = msg or false
      error(20)
    end)
    local y <close> = func2close(function(_,msg)
      track[#track + 1] = msg or false
      return 1000
    end)
    local z <close> = func2close(function(_,msg)
      track[#track + 1] = msg or false
      error(10)
    end)
    coroutine.yield(1)
    h(func2close(function(_,msg)
        track[#track + 1] = msg or false
        error(2)
      end))
  end

  local co = coroutine.create(pcall)

  local st, res = coroutine.resume(co, foo)    -- call 'foo' protected
  assert(st and res == 1)   -- yield 1
  local st, res1, res2 = coroutine.resume(co)   -- continue
  assert(coroutine.status(co) == "dead")
  assert(st and not res1 and res2 == 20)   -- last error (20)
  assert(track[1] == false and track[2] == 2 and track[3] == 10 and
         track[4] == 10)
end


-- yielding across C boundaries

local co = coroutine.wrap(function()
       assert(not pcall(table.sort,{1,2,3}, coroutine.yield))
       assert(coroutine.isyieldable())
       coroutine.yield(20)
       return 30
     end)

assert(co() == 20)
assert(co() == 30)


local f = function (s, i) return coroutine.yield(i) end

local f1 = coroutine.wrap(function ()
             return xpcall(pcall, function (...) return ... end,
               function ()
                 local s = 0
                 for i in f, nil, 1 do pcall(function () s = s + i end) end
                 error({s})
               end)
           end)

f1()
for i = 1, 10 do assert(f1(i) == i) end
local r1, r2, v = f1(nil)
assert(r1 and not r2 and v[1] ==  (10 + 1)*10/2)


local function f (a, b) a = coroutine.yield(a);  error{a + b} end
local function g(x) return x[1]*2 end

co = coroutine.wrap(function ()
       coroutine.yield(xpcall(f, g, 10, 20))
     end)

assert(co() == 10)
local r, msg = co(100)
assert(not r and msg == 240)


-- unyieldable C call
do
  local function f (c)
          assert(not coroutine.isyieldable())
          return c .. c
        end

  local co = coroutine.wrap(function (c)
               assert(coroutine.isyieldable())
               local s = string.gsub("a", ".", f)
               return s
             end)
  assert(co() == "aa")
end

-- errors in coroutines
function foo ()
  coroutine.yield(3)
  error(foo)
end

function goo() foo() end
x = coroutine.wrap(goo)
assert(x() == 3)
local a,b = pcall(x)
assert(not a and b == foo)

x = coroutine.create(goo)
a,b = coroutine.resume(x)
assert(a and b == 3)
a,b = coroutine.resume(x)
assert(not a and b == foo and coroutine.status(x) == "dead")
a,b = coroutine.resume(x)
assert(not a and string.find(b, "dead") and coroutine.status(x) == "dead")

goo = nil

-- co-routines x for loop
local function all (a, n, k)
  if k == 0 then coroutine.yield(a)
  else
    for i=1,n do
      a[k] = i
      all(a, n, k-1)
    end
  end
end

local a = 0
for t in coroutine.wrap(function () all({}, 5, 4) end) do
  a = a+1
end
assert(a == 5^4)


-- access to locals of collected corroutines
local C = {}; setmetatable(C, {__mode = "kv"})
local x = coroutine.wrap (function ()
            local a = 10
            local function f () a = a+10; return a end
            while true do
              a = a+1
              coroutine.yield(f)
            end
          end)

C[1] = x;

local f = x()
assert(f() == 21 and x()() == 32 and x() == f)
x = nil
collectgarbage()
assert(C[1] == undef)
assert(f() == 43 and f() == 53)


-- old bug: attempt to resume itself

local function co_func (current_co)
  assert(coroutine.running() == current_co)
  assert(coroutine.resume(current_co) == false)
  coroutine.yield(10, 20)
  assert(coroutine.resume(current_co) == false)
  coroutine.yield(23)
  return 10
end

local co = coroutine.create(co_func)
local a,b,c = coroutine.resume(co, co)
assert(a == true and b == 10 and c == 20)
a,b = coroutine.resume(co, co)
assert(a == true and b == 23)
a,b = coroutine.resume(co, co)
assert(a == true and b == 10)
assert(coroutine.resume(co, co) == false)
assert(coroutine.resume(co, co) == false)


-- other old bug when attempting to resume itself
-- (trigger C-code assertions)
do
  local A = coroutine.running()
  local B = coroutine.create(function() return coroutine.resume(A) end)
  local st, res = coroutine.resume(B)
  assert(st == true and res == false)

  local X = false
  A = coroutine.wrap(function()
    local _ <close> = func2close(function () X = true end)
    return pcall(A, 1)
  end)
  st, res = A()
  assert(not st and string.find(res, "non%-suspended") and X == true)
end


-- bug in 5.4.1
do
  -- coroutine ran close metamethods with invalid status during a
  -- reset.
  local co
  co = coroutine.wrap(function()
    local x <close> = func2close(function() return pcall(co) end)
    error(111)
  end)
  local st, errobj = pcall(co)
  assert(not st and errobj == 111)
  st, errobj = pcall(co)
  assert(not st and string.find(errobj, "dead coroutine"))
end


-- attempt to resume 'normal' coroutine
local co1, co2
co1 = coroutine.create(function () return co2() end)
co2 = coroutine.wrap(function ()
        assert(coroutine.status(co1) == 'normal')
        assert(not coroutine.resume(co1))
        coroutine.yield(3)
      end)

a,b = coroutine.resume(co1)
assert(a and b == 3)
assert(coroutine.status(co1) == 'dead')

-- infinite recursion of coroutines
a = function(a) coroutine.wrap(a)(a) end
assert(not pcall(a, a))
a = nil


do
  -- bug in 5.4: thread can use message handler higher in the stack
  -- than the variable being closed
  local c = coroutine.create(function()
    local clo <close> = setmetatable({}, {__close=function()
      local x = 134   -- will overwrite message handler
      error(x)
    end})
    -- yields coroutine but leaves a new message handler for it,
    -- that would be used when closing the coroutine (except that it
    -- will be overwritten)
    xpcall(coroutine.yield, function() return "XXX" end)
  end)

  assert(coroutine.resume(c))   -- start coroutine
  local st, msg = coroutine.close(c)
  assert(not st and msg == 134)
end

-- access to locals of erroneous coroutines
local x = coroutine.create (function ()
            local a = 10
            _G.F = function () a=a+1; return a end
            error('x')
          end)

assert(not coroutine.resume(x))
-- overwrite previous position of local `a'
assert(not coroutine.resume(x, 1, 1, 1, 1, 1, 1, 1))
assert(_G.F() == 11)
assert(_G.F() == 12)
_G.F = nil

-- leaving a pending coroutine open
_G.TO_SURVIVE = coroutine.wrap(function ()
      local a = 10
      local x = function () a = a+1 end
      coroutine.yield()
    end)

_G.TO_SURVIVE()


if not _soft then
  -- bug (stack overflow)
  local lim = 1000000    -- stack limit; assume 32-bit machine
  local t = {lim - 10, lim - 5, lim - 1, lim, lim + 1, lim + 5}
  for i = 1, #t do
    local j = t[i]
    local co = coroutine.create(function()
           return table.unpack({}, 1, j)
         end)
    local r, msg = coroutine.resume(co)
    -- must fail for unpacking larger than stack limit
    assert(j < lim or not r)
  end
end


assert(coroutine.running() == main)

print"+"


print"testing yields inside metamethods"

local function val(x)
  if type(x) == "table" then return x.x else return x end
end

local mt = {
  __eq = function(a,b) coroutine.yield(nil, "eq"); return val(a) == val(b) end,
  __lt = function(a,b) coroutine.yield(nil, "lt"); return val(a) < val(b) end,
  __le = function(a,b) coroutine.yield(nil, "le"); return a - b <= 0 end,
  __add = function(a,b) coroutine.yield(nil, "add");
                        return val(a) + val(b) end,
  __sub = function(a,b) coroutine.yield(nil, "sub"); return val(a) - val(b) end,
  __mul = function(a,b) coroutine.yield(nil, "mul"); return val(a) * val(b) end,
  __div = function(a,b) coroutine.yield(nil, "div"); return val(a) / val(b) end,
  __idiv = function(a,b) coroutine.yield(nil, "idiv");
                         return val(a) // val(b) end,
  __pow = function(a,b) coroutine.yield(nil, "pow"); return val(a) ^ val(b) end,
  __mod = function(a,b) coroutine.yield(nil, "mod"); return val(a) % val(b) end,
  __unm = function(a,b) coroutine.yield(nil, "unm"); return -val(a) end,
  __bnot = function(a,b) coroutine.yield(nil, "bnot"); return ~val(a) end,
  __shl = function(a,b) coroutine.yield(nil, "shl");
                        return val(a) << val(b) end,
  __shr = function(a,b) coroutine.yield(nil, "shr");
                        return val(a) >> val(b) end,
  __band = function(a,b)
             coroutine.yield(nil, "band")
             return val(a) & val(b)
           end,
  __bor = function(a,b) coroutine.yield(nil, "bor");
                        return val(a) | val(b) end,
  __bxor = function(a,b) coroutine.yield(nil, "bxor");
                         return val(a) ~ val(b) end,

  __concat = function(a,b)
               coroutine.yield(nil, "concat");
               return val(a) .. val(b)
             end,
  __index = function (t,k) coroutine.yield(nil, "idx"); return t.k[k] end,
  __newindex = function (t,k,v) coroutine.yield(nil, "nidx"); t.k[k] = v end,
}


local function new (x)
  return setmetatable({x = x, k = {}}, mt)
end


local a = new(10)
local b = new(12)
local c = new"hello"

local function run (f, t)
  local i = 1
  local c = coroutine.wrap(f)
  while true do
    local res, stat = c()
    if res then assert(t[i] == undef); return res, t end
    assert(stat == t[i])
    i = i + 1
  end
end


assert(run(function () if (a>=b) then return '>=' else return '<' end end,
       {"le", "sub"}) == "<")
assert(run(function () if (a<=b) then return '<=' else return '>' end end,
       {"le", "sub"}) == "<=")
assert(run(function () if (a==b) then return '==' else return '~=' end end,
       {"eq"}) == "~=")

assert(run(function () return a & b + a end, {"add", "band"}) == 2)

assert(run(function () return 1 + a end, {"add"}) == 11)
assert(run(function () return a - 25 end, {"sub"}) == -15)
assert(run(function () return 2 * a end, {"mul"}) == 20)
assert(run(function () return a ^ 2 end, {"pow"}) == 100)
assert(run(function () return a / 2 end, {"div"}) == 5)
assert(run(function () return a % 6 end, {"mod"}) == 4)
assert(run(function () return a // 3 end, {"idiv"}) == 3)

assert(run(function () return a + b end, {"add"}) == 22)
assert(run(function () return a - b end, {"sub"}) == -2)
assert(run(function () return a * b end, {"mul"}) == 120)
assert(run(function () return a ^ b end, {"pow"}) == 10^12)
assert(run(function () return a / b end, {"div"}) == 10/12)
assert(run(function () return a % b end, {"mod"}) == 10)
assert(run(function () return a // b end, {"idiv"}) == 0)

-- repeat tests with larger constants (to use 'K' opcodes)
local a1000 = new(1000)

assert(run(function () return a1000 + 1000 end, {"add"}) == 2000)
assert(run(function () return a1000 - 25000 end, {"sub"}) == -24000)
assert(run(function () return 2000 * a end, {"mul"}) == 20000)
assert(run(function () return a1000 / 1000 end, {"div"}) == 1)
assert(run(function () return a1000 % 600 end, {"mod"}) == 400)
assert(run(function () return a1000 // 500 end, {"idiv"}) == 2)



assert(run(function () return a % b end, {"mod"}) == 10)

assert(run(function () return ~a & b end, {"bnot", "band"}) == ~10 & 12)
assert(run(function () return a | b end, {"bor"}) == 10 | 12)
assert(run(function () return a ~ b end, {"bxor"}) == 10 ~ 12)
assert(run(function () return a << b end, {"shl"}) == 10 << 12)
assert(run(function () return a >> b end, {"shr"}) == 10 >> 12)

assert(run(function () return 10 & b end, {"band"}) == 10 & 12)
assert(run(function () return a | 2 end, {"bor"}) == 10 | 2)
assert(run(function () return a ~ 2 end, {"bxor"}) == 10 ~ 2)
assert(run(function () return a >> 2 end, {"shr"}) == 10 >> 2)
assert(run(function () return 1 >> a end, {"shr"}) == 1 >> 10)
assert(run(function () return a << 2 end, {"shl"}) == 10 << 2)
assert(run(function () return 1 << a end, {"shl"}) == 1 << 10)
assert(run(function () return 2 ~ a end, {"bxor"}) == 2 ~ 10)


assert(run(function () return a..b end, {"concat"}) == "1012")

assert(run(function() return a .. b .. c .. a end,
       {"concat", "concat", "concat"}) == "1012hello10")

assert(run(function() return "a" .. "b" .. a .. "c" .. c .. b .. "x" end,
       {"concat", "concat", "concat"}) == "ab10chello12x")


do   -- a few more tests for comparison operators
  local mt1 = {
    __le = function (a,b)
      coroutine.yield(10)
      return (val(a) <= val(b))
    end,
    __lt = function (a,b)
      coroutine.yield(10)
      return val(a) < val(b)
    end,
  }
  local mt2 = { __lt = mt1.__lt, __le = mt1.__le }

  local function run (f)
    local co = coroutine.wrap(f)
    local res
    repeat
      res = co()
    until res ~= 10
    return res
  end

  local function test ()
    local a1 = setmetatable({x=1}, mt1)
    local a2 = setmetatable({x=2}, mt2)
    assert(a1 < a2)
    assert(a1 <= a2)
    assert(1 < a2)
    assert(1 <= a2)
    assert(2 > a1)
    assert(2 >= a2)
    return true
  end

  run(test)

end

assert(run(function ()
             a.BB = print
             return a.BB
           end, {"nidx", "idx"}) == print)

print"+"

print"testing yields inside 'for' iterators"

local f = function (s, i)
      if i%2 == 0 then coroutine.yield(nil, "for") end
      if i < s then return i + 1 end
    end

assert(run(function ()
             local s = 0
             for i in f, 4, 0 do s = s + i end
             return s
           end, {"for", "for", "for"}) == 10)

print "OK"
