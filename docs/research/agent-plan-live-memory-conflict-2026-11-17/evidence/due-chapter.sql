UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp() WHERE work_kind = 'chapter_capture' AND chapter_id = '01a1ea84-14b3-79c1-9761-84e6a41742a8' AND NOT held
