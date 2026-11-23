-- The request guard of the live check of #435. It refuses the claim of a model request above 30 of one Run of the
-- author (and of each request of a Run of another kind), a model request above 40 in all, an AgentRun above 3 in all,
-- and each Exa or embedding request. It changes no product row.
CREATE SCHEMA live_guard;
CREATE TABLE live_guard.cap (kind text PRIMARY KEY, cap integer NOT NULL, used integer NOT NULL DEFAULT 0);
INSERT INTO live_guard.cap (kind, cap) VALUES ('model', 40), ('run', 3), ('exa', 0), ('embedding', 0);
CREATE TABLE live_guard.log (at timestamptz NOT NULL DEFAULT clock_timestamp(), kind text NOT NULL, ref uuid, role text, position numeric);
CREATE FUNCTION live_guard.count_model() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = live_guard, pg_catalog AS $$
DECLARE c live_guard.cap; run_cap integer;
BEGIN
  SELECT CASE run_kind WHEN 'author' THEN 30 ELSE 0 END INTO run_cap
    FROM storyos.agent_runs WHERE run_id = NEW.run_id;
  IF (SELECT count(*) FROM storyos.model_attempts WHERE run_id = NEW.run_id) >= coalesce(run_cap, 0) THEN
    RAISE EXCEPTION 'live_guard: model request cap of one Run reached';
  END IF;
  UPDATE live_guard.cap SET used = used + 1 WHERE kind = 'model' RETURNING * INTO c;
  IF c.used > c.cap THEN
    RAISE EXCEPTION 'live_guard: model request cap % reached', c.cap;
  END IF;
  INSERT INTO live_guard.log (kind, ref, role, position) VALUES ('model', NEW.model_attempt_id, NEW.attempt_role, NEW.decision_position);
  RETURN NEW;
END $$;
CREATE FUNCTION live_guard.count_run() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = live_guard, pg_catalog AS $$
DECLARE c live_guard.cap;
BEGIN
  UPDATE live_guard.cap SET used = used + 1 WHERE kind = 'run' RETURNING * INTO c;
  IF c.used > c.cap THEN
    RAISE EXCEPTION 'live_guard: AgentRun cap % reached', c.cap;
  END IF;
  INSERT INTO live_guard.log (kind, ref, role) VALUES ('run', NEW.run_id, NEW.run_kind);
  RETURN NEW;
END $$;
CREATE FUNCTION live_guard.count_exa() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = live_guard, pg_catalog AS $$
DECLARE c live_guard.cap;
BEGIN
  IF NEW.destination_attempt_id IS NOT NULL AND (TG_OP = 'INSERT' OR OLD.destination_attempt_id IS NULL) THEN
    UPDATE live_guard.cap SET used = used + 1 WHERE kind = 'exa' RETURNING * INTO c;
    IF c.used > c.cap THEN
      RAISE EXCEPTION 'live_guard: Exa exchange cap % reached', c.cap;
    END IF;
    INSERT INTO live_guard.log (kind, ref, role, position) VALUES ('exa', NEW.tool_call_id, NEW.function_name, NEW.decision_position);
  END IF;
  RETURN NEW;
END $$;
CREATE FUNCTION live_guard.count_embedding() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = live_guard, pg_catalog AS $$
DECLARE c live_guard.cap;
BEGIN
  UPDATE live_guard.cap SET used = used + 1 WHERE kind = 'embedding' RETURNING * INTO c;
  IF c.used > c.cap THEN
    RAISE EXCEPTION 'live_guard: embedding request cap % reached', c.cap;
  END IF;
  INSERT INTO live_guard.log (kind, ref, role) VALUES ('embedding', NEW.embedding_attempt_id, NEW.cause);
  RETURN NEW;
END $$;
CREATE TRIGGER live_guard_model BEFORE INSERT ON storyos.model_attempts FOR EACH ROW EXECUTE FUNCTION live_guard.count_model();
CREATE TRIGGER live_guard_run BEFORE INSERT ON storyos.agent_runs FOR EACH ROW EXECUTE FUNCTION live_guard.count_run();
CREATE TRIGGER live_guard_exa_insert BEFORE INSERT ON storyos.tool_calls FOR EACH ROW EXECUTE FUNCTION live_guard.count_exa();
CREATE TRIGGER live_guard_exa_update BEFORE UPDATE OF destination_attempt_id ON storyos.tool_calls FOR EACH ROW EXECUTE FUNCTION live_guard.count_exa();
CREATE TRIGGER live_guard_embedding BEFORE INSERT ON storyos.embedding_attempts FOR EACH ROW EXECUTE FUNCTION live_guard.count_embedding();
