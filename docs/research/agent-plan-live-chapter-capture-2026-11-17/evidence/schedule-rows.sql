-- Read-only query of the schedule rows of the Project, at three times (UTC).
SELECT work_kind, source_id, held, due_at, clock_timestamp() FROM storyos.memory_work_schedules WHERE project_id = '01a1ea65-160a-7a23-82d0-70bef27dfc1c';
-- after the commit of the Chapter text, before the due time:
-- chapter_capture|01a1ea66-0cb3-7e32-ab7e-edfb64638b79|f|2026-11-17 17:45:46.958149+00|2026-11-17 16:46:06.993014+00
-- after the claim at the due time, before the Run of the author:
-- chapter_capture|01a1ea66-0cb3-7e32-ab7e-edfb64638b79|t||2026-11-17 17:46:22.467337+00
-- after the Run of the author completed:
-- conversation_capture|01a1ea9e-5837-7931-9c89-cd5895b4aeeb|f|2026-11-17 18:17:36.943408+00|2026-11-17 17:47:38.607375+00
-- chapter_capture|01a1ea66-0cb3-7e32-ab7e-edfb64638b79|f||2026-11-17 17:47:38.60738+00
