-- Transposes every chart of a set: `args.paths` (the charts' paths in the
-- vault) moved `args.by` semitones (2 up when it is not given). Each
-- chart is one edit, so one Undo on a chart brings it back.
--
-- Run it with SCRIPT_RUN (⌘K, the CLI, MCP):
--   { paths = { "Libraries/Christmas/Feliz Navidad/Feliz Navidad.kf", … }, by = -2 }
--
-- It reads each chart's structure first (KEYFLOW_DESCRIBE) to know how
-- many bars it has, and says what it left alone: a number chart's degrees
-- transpose by their key, not by their chords.
local by = if args and args.by then args.by else 2
local done = {}

for _, path in (args and args.paths) or {} do
  local chart = state.query("KEYFLOW_DESCRIBE", { path = path })
  local bars = #chart.bars
  if bars == 0 then
    print("no bars:", path)
    continue
  end
  local out = actions.run("KEYFLOW_EDIT", {
    path = path,
    steps = {
      { select = { bars = { 1, bars } }, action = "transpose", by = by },
    },
  })
  print(path, "chords moved:", #out.changed, "left alone:", #out.skipped)
  table.insert(done, { path = path, changed = #out.changed, skipped = out.skipped })
end

return done
