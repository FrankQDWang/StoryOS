-- C1 2026-11-18T07:54:50.732Z: 1 rows, planned 1
UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '01a1eda5-6c9e-7af0-9aa5-b9147837fbc4'::uuid AND work_kind = 'conversation_capture' AND source_id = '01a1eda6-590c-70a1-904e-6db4f95fb7e3'::uuid;
