-- Deployment-owned OAuth client; public metadata only. / 部署所有的 OAuth 客户端，仅含公开元数据。
-- Apply as a reviewed forward-only D1 migration, not as separate CLI statements. / 作为已评审的只向前 D1 migration 应用，不得逐条执行。
INSERT INTO oauth_clients(client_id,display_name,client_type,token_endpoint_auth_method,sector_identifier,subject_salt_revision,state,created_at,updated_at) VALUES('mskill-cli','mskill CLI','native','none','skills.moesegfault.dev',1,'enabled',unixepoch(),unixepoch());
INSERT INTO oauth_redirect_uris(redirect_uri_id,client_id,redirect_uri,match_mode,created_at) VALUES('01a10b08-71f5-7aff-a922-2696bb8b7b23','mskill-cli','http://127.0.0.1/callback','native_loopback_any_port',unixepoch());
INSERT INTO oauth_client_scopes(client_id,scope,granted_at) VALUES('mskill-cli','openid',unixepoch());
INSERT INTO oauth_client_scopes(client_id,scope,granted_at) VALUES('mskill-cli','offline_access',unixepoch());
INSERT INTO security_audit_events(audit_event_id,event_name,occurred_at,observed_at,client_id,outcome,correlation_id,policy_revision,context_json) VALUES('01a10b08-71f5-751a-9b81-416be17c5cdb','identity.oauth_client.created',unixepoch(),unixepoch(),'mskill-cli','success','01a10b08-71f5-751a-9b81-416be17c5cdb',1,json_object('client_id','mskill-cli'));
INSERT INTO audit_archive_outbox(audit_event_id,r2_object_key,next_attempt_at) VALUES('01a10b08-71f5-751a-9b81-416be17c5cdb',printf('security-audit/unix-day-%d/%s.json',CAST(unixepoch()/86400 AS INTEGER),'01a10b08-71f5-751a-9b81-416be17c5cdb'),unixepoch());
