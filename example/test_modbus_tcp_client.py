"""交互范围写入测试，不连接设备：python -m unittest discover -s example。"""
import contextlib
import io
import unittest
from unittest.mock import Mock, patch

from modbus_tcp_client import command_mode


class CommandsTest(unittest.TestCase):
    def run_commands(self, commands):
        client = Mock()
        client.write_registers.return_value.isError.return_value = False
        client.write_coils.return_value.isError.return_value = False
        with patch('builtins.input', side_effect=commands), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(command_mode(client, 1), 0)
        return client

    def test_uniform_register_range(self):
        client = self.run_commands(['yt', '1000-1002', '25', 'q'])
        client.write_registers.assert_called_once_with(1000, [25, 25, 25], device_id=1)
        client.write_register.assert_not_called()

    def test_coil_list_sends_immediately(self):
        client = self.run_commands(['yk', '10-12', '0,1,0', 'q'])
        client.write_coils.assert_called_once_with(10, [False, True, False], device_id=1)

    def test_invalid_range_and_values_do_not_send(self):
        client = self.run_commands(['yt', '3-1', '65535-65536', '0-123', '1000-1002',
                                    '1,2', '1,2,65536', '1,2,3', 'q'])
        client.write_registers.assert_called_once_with(1000, [1, 2, 3], device_id=1)

    def test_range_read(self):
        client = self.run_commands(['read', '1-3', 'q'])
        client.read_holding_registers.assert_called_once_with(1, count=3, device_id=1)


if __name__ == '__main__':
    unittest.main()
