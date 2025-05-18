local ok = false
local mt = {
  __close = function (obj, err)
    assert(type(obj) == 'table')
    assert(type(err) == 'nil')
    ok = true
  end
}

local function foo()
  local bar <close> = setmetatable({}, mt)
end

foo()
assert(ok)
