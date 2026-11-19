-- Reads a chart, decides, writes: every chord written on beat 3 of a bar
-- of 4/4 that changes chord there (two chords in the bar) is pushed an
-- eighth, the way a band plays "the and of two". One edit, one undo step.
--
--   { path = "Libraries/…/Song.kf" }   (the active chart without it)
--
-- A chord already pushed is left as it is; so is anything the action
-- refuses (it says why in `skipped`).
local path = args and args.path
local chart = state.query("KEYFLOW_DESCRIBE", { path = path })

local steps = {}
for _, bar in chart.bars do
  if bar.meter[1] == 4 and bar.meter[2] == 4 and #bar.chords == 2 then
    local second = bar.chords[2]
    if second.beat == 3 and second.push == nil then
      table.insert(steps, {
        select = { bar = bar.bar, beat = 3 },
        action = "push",
        step = "eighth",
      })
    end
  end
end

if #steps == 0 then
  return { pushed = 0 }
end
local out = actions.run("KEYFLOW_EDIT", { path = chart.path, steps = steps })
return { pushed = #out.changed, skipped = out.skipped }
