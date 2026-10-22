"""串口选择与命令边界，不访问真实设备。"""
import contextlib
import io
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch
from modbus_rtu_client import select_port, command_mode, main

class SerialClientTest(unittest.TestCase):
    def test_refresh_select_and_manual_path(self):
        listing=Mock(side_effect=[[],[SimpleNamespace(device='/dev/cu.fixture',description='RS485',manufacturer='Vendor',serial_number='SN')]])
        with patch('builtins.input',side_effect=['r','1']),contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(select_port(listing),'/dev/cu.fixture')
        self.assertEqual(listing.call_count,2)
        with patch('builtins.input',return_value='/dev/ttys999'),contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(select_port(lambda:[]),'/dev/ttys999')

    def test_range_write_and_four_area_reads_without_extra_confirmation(self):
        client=Mock();client.read_holding_registers.return_value.isError.return_value=False
        client.read_holding_registers.return_value.registers=[7,8]
        for name in ['read_coils','read_discrete_inputs','read_input_registers']:
            getattr(client,name).return_value.isError.return_value=False
            getattr(client,name).return_value.bits=[True]*8
            getattr(client,name).return_value.registers=[9]
        client.write_coils.return_value.isError.return_value=False
        client.write_registers.return_value.isError.return_value=False
        with patch('builtins.input',side_effect=['yk','100-103','1','yt','100-101','7,8','read','100-101','rc','0','rd','0','ri','0','q']),contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(command_mode(client,1),0)
        client.write_coils.assert_called_once_with(100,[True]*4,device_id=1)
        client.write_registers.assert_called_once_with(100,[7,8],device_id=1)
        client.read_holding_registers.assert_called_once_with(100,count=2,device_id=1)
        client.read_input_registers.assert_called_once_with(0,count=1,device_id=1)
        client.read_coils.assert_called_once_with(0,count=1,device_id=1)
        client.read_discrete_inputs.assert_called_once_with(0,count=1,device_id=1)

    def test_write_timeout_is_not_repeated(self):
        from pymodbus.exceptions import ModbusException
        client=Mock();client.write_register.side_effect=ModbusException('timeout')
        with patch('builtins.input',side_effect=['yt','0','42']),contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(command_mode(client,1),1)
        client.write_register.assert_called_once_with(0,42,device_id=1)

    def test_broadcast_and_invalid_timeout_rejected_before_open(self):
        for args in [['--unit','0'],['--timeout','nan'],['--baud','0']]:
            with self.subTest(args=args),contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):main(args)

if __name__=='__main__':unittest.main()
