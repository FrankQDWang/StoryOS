-- C1 2026-11-17T23:28:11.564Z: 1 rows, planned 1
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1ebd5-9085-7140-b493-1c48f786df9d'::uuid AND work_kind = 'conversation_capture' AND source_id = '01a1ebd6-7f00-76d3-8c92-fa9840b6efd1'::uuid;
-- A1 2026-11-17T23:28:27.519Z: 3 rows, planned 3
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1ebd5-9085-7140-b493-1c48f786df9d'::uuid AND work_kind = 'chapter_capture' AND chapter_id IN ('01a1ebd5-9156-78d1-bb31-371855f7651d'::uuid, '01a1ebd5-916c-7581-85e5-742bae021692'::uuid, '01a1ebd5-9186-73b1-82ea-822db562cb69'::uuid);
-- B1 2026-11-17T23:33:26.803Z: 1 rows, planned 1
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1ebd5-9085-7140-b493-1c48f786df9d'::uuid AND work_kind = 'consolidation';
-- A2 2026-11-17T23:36:45.642Z: 1 rows, planned 1
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1ebd5-9085-7140-b493-1c48f786df9d'::uuid AND work_kind = 'chapter_capture' AND chapter_id = '01a1ebd5-9186-73b1-82ea-822db562cb69'::uuid;
-- B2 2026-11-17T23:38:20.068Z: 1 rows, planned 1
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1ebd5-9085-7140-b493-1c48f786df9d'::uuid AND work_kind = 'consolidation';
