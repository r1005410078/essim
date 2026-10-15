#include <modbus/modbus.h>
#include <errno.h>
#include <stddef.h>
#if LIBMODBUS_VERSION_MAJOR != 3 || LIBMODBUS_VERSION_MINOR != 2 || LIBMODBUS_VERSION_MICRO != 0
#error libmodbus-sys requires libmodbus 3.2.0
#endif
_Static_assert(MODBUS_TCP_MAX_ADU_LENGTH == 260, "TCP ADU size mismatch");
_Static_assert(MODBUS_RTU_MAX_ADU_LENGTH == 256, "RTU ADU size mismatch");
_Static_assert(MODBUS_ENOBASE == 112345678, "protocol errno base mismatch");
_Static_assert(MODBUS_MAX_READ_BITS == 2000 && MODBUS_MAX_WRITE_BITS == 1968, "bit limits mismatch");
_Static_assert(MODBUS_MAX_READ_REGISTERS == 125 && MODBUS_MAX_WRITE_REGISTERS == 123, "register limits mismatch");
_Static_assert(sizeof(uint16_t) == 2, "register ABI mismatch");
/* Header-checked ABI probes for every imported function. */
static void probe(void) {
    int (*read_bits_fn)(modbus_t *, int, int, uint8_t *) = modbus_read_bits; (void)read_bits_fn;
    int (*read_input_bits_fn)(modbus_t *, int, int, uint8_t *) = modbus_read_input_bits; (void)read_input_bits_fn;
    int (*read_registers_fn)(modbus_t *, int, int, uint16_t *) = modbus_read_registers; (void)read_registers_fn;
    int (*read_input_registers_fn)(modbus_t *, int, int, uint16_t *) = modbus_read_input_registers; (void)read_input_registers_fn;
    int (*write_bits_fn)(modbus_t *, int, int, const uint8_t *) = modbus_write_bits; (void)write_bits_fn;
    int (*write_registers_fn)(modbus_t *, int, int, const uint16_t *) = modbus_write_registers; (void)write_registers_fn;
    int (*write_bit_fn)(modbus_t *, int, int) = modbus_write_bit; (void)write_bit_fn;
    int (*write_register_fn)(modbus_t *, int, uint16_t) = modbus_write_register; (void)write_register_fn;
    modbus_t *(*a)(const char *, int) = modbus_new_tcp;
    int (*b)(modbus_t *, int) = modbus_set_socket;
    int (*c)(modbus_t *, uint32_t, uint32_t) = modbus_set_indication_timeout;
    int (*d)(modbus_t *, uint32_t, uint32_t) = modbus_set_byte_timeout;
    void (*e)(modbus_t *) = modbus_free;
    modbus_mapping_t *(*f)(unsigned int,unsigned int,unsigned int,unsigned int,unsigned int,unsigned int,unsigned int,unsigned int) = modbus_mapping_new_start_address;
    void (*g)(modbus_mapping_t *) = modbus_mapping_free;
    int (*h)(modbus_t *, uint8_t *) = modbus_receive;
    int (*i)(modbus_t *, const uint8_t *, int, modbus_mapping_t *) = modbus_reply;
    int (*j)(modbus_t *, const uint8_t *, unsigned int) = modbus_reply_exception;
    modbus_t *(*rtu)(const char *, int, char, int, int) = modbus_new_rtu;
    int (*connect_fn)(modbus_t *) = modbus_connect;
    int (*slave_fn)(modbus_t *, int) = modbus_set_slave;
    void (*close_fn)(modbus_t *) = modbus_close;
    int (*response_fn)(modbus_t *, uint32_t, uint32_t) = modbus_set_response_timeout;
    (void)rtu;(void)connect_fn;(void)slave_fn;(void)close_fn;(void)response_fn;
    (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;(void)g;(void)h;(void)i;(void)j;
}
int libmodbus_rs_register(modbus_mapping_t *m, unsigned int address, uint16_t *value, int write) {
    probe();
    if (!m || !value || !m->tab_registers || address < (unsigned int)m->start_registers || address - (unsigned int)m->start_registers >= (unsigned int)m->nb_registers) {errno=EINVAL;return -1;}
    if (write) m->tab_registers[address-m->start_registers]=*value;
    else *value=m->tab_registers[address-m->start_registers];
    return 0;
}

/* Project helper: area 0=coils, 1=discrete inputs, 2=holding, 3=input registers.
 * Local server access may update every area; network permissions are handled by libmodbus. */
int libmodbus_rs_value(modbus_mapping_t *m, unsigned int area, unsigned int address, uint16_t *value, int write) {
    uint8_t *bits = NULL;
    uint16_t *registers = NULL;
    unsigned int start, count;
    if (!m || !value) { errno = EINVAL; return -1; }
    switch (area) {
    case 0: bits=m->tab_bits; start=m->start_bits; count=m->nb_bits; break;
    case 1: bits=m->tab_input_bits; start=m->start_input_bits; count=m->nb_input_bits; break;
    case 2: registers=m->tab_registers; start=m->start_registers; count=m->nb_registers; break;
    case 3: registers=m->tab_input_registers; start=m->start_input_registers; count=m->nb_input_registers; break;
    default: errno=EINVAL; return -1;
    }
    if ((!bits && !registers) || address < start || address-start >= count || (bits && write && *value > 1)) {
        errno=EINVAL; return -1;
    }
    unsigned int offset = address-start;
    if (write) {
        if (bits) bits[offset]=(uint8_t)*value;
        else registers[offset]=*value;
    } else {
        *value=bits ? bits[offset] : registers[offset];
    }
    return 0;
}
