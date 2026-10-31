"""Existing native screen harness wire/owner checks; no native authority proof."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import Mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('screen_acceptance', ROOT / 'scripts/screen-acceptance.py')
SCREEN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCREEN)


class ScreenOrganizationContext(unittest.TestCase):
    def setUp(self):
        self.actor = {'user': 'synthetic-user', 'company': 'synthetic-company',
                      'membership_revision': 7}
        self.context = {'principal_id': self.actor['user'],
                        'space': {'kind': 'organization', 'id': self.actor['company']},
                        'membership_revision': 7}
        self.authority = SCREEN.Authority(
            {'base_url': 'http://127.0.0.1:1', 'mailpit_url': 'http://127.0.0.1:1'},
            Path('/unused-synthetic-scope'))

    def test_read_uses_the_original_organization_and_revision(self):
        reply = {'context': self.context, 'members': []}
        self.authority.signed = Mock(return_value=(200, reply))
        self.assertIs(self.authority.scoped(self.actor, '/members'), reply)
        self.authority.signed.assert_called_once_with(
            self.actor, '/v3/organizations/synthetic-company/members?membership_revision=7', None)

    def test_write_wraps_the_original_revision_without_refreshing_or_replaying(self):
        payload = {'operation_id': 'synthetic-operation', 'email': 'member@example.invalid', 'role': 'user'}
        reply = {'context': self.context, 'invitation': {}}
        self.authority.signed = Mock(return_value=(201, reply))
        self.assertIs(self.authority.scoped(self.actor, '/invitations', payload, expected=201), reply)
        self.authority.signed.assert_called_once_with(
            self.actor, '/v3/organizations/synthetic-company/invitations',
            {'membership_revision': 7, 'payload': payload})
        self.authority.signed = Mock(return_value=(200, reply))
        with self.assertRaises(AssertionError):
            self.authority.scoped(self.actor, '/invitations', payload, expected=201)
        self.assertEqual(self.authority.signed.call_count, 1)
        self.assertEqual(self.actor['membership_revision'], 7)

    def test_foreign_or_new_context_never_publishes_a_reply(self):
        for context in [self.context | {'principal_id': 'another-user'},
                        self.context | {'membership_revision': 8},
                        self.context | {'space': {'kind': 'organization', 'id': 'another-company'}},
                        self.context | {'space': {'kind': 'personal', 'id': self.actor['user']}}]:
            with self.subTest(context=context):
                self.authority.signed = Mock(return_value=(200, {'context': context, 'members': []}))
                with self.assertRaises(AssertionError):
                    self.authority.scoped(self.actor, '/members')
                self.assertEqual(self.authority.signed.call_count, 1)

    def test_fresh_diagnostic_read_keeps_the_original_company_and_command_unchanged(self):
        row = {'company': {'company_id': self.actor['company']},
               'member': {'user_id': self.actor['user'], 'active': True, 'revision': 8}}
        other = row | {'company': {'company_id': 'another-company'}}
        personal = {'principal_id': self.actor['user'],
                    'space': {'kind': 'personal', 'id': self.actor['user']}}
        reply = {'context': personal, 'organizations': [other, row]}
        self.authority.signed = Mock(return_value=(200, reply))
        self.assertIs(self.authority.current_organization(self.actor), row)
        self.assertEqual(self.actor['membership_revision'], 7)
        fresh = {'user': self.actor['user']}
        with self.assertRaises(AssertionError):
            self.authority.current_organization(fresh)
        reply['context'] = personal | {'principal_id': 'another-user'}
        with self.assertRaises(AssertionError):
            self.authority.current_organization(self.actor)
