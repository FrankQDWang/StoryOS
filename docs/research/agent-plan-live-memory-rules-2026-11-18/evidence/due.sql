-- C1 2026-11-18T07:30:28.940Z: 1 rows, planned 1
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1ed8f-0a2d-71f1-8492-fdb6e6b2b3c1'::uuid AND work_kind = 'conversation_capture' AND source_id = '01a1ed8f-f694-7dc0-81b3-2b7511851828'::uuid;
