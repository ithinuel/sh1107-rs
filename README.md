# Async Driver for the SH1107 − 128 X 128 Dot Matrix OLED/PLED and related integrations

The examples available in this repository are supported on the following target:

- Sparkfun pro-micro 2040
- Pimoroni pico-explorer

Note that because of limitations of the power supply, several reset (without power cycle) may be
required for the display to turn on.  
This can be worked around by adding an extra capacitance between 3.3V and GND. The Sparkfun pro-micro
2040 typically requires a value around 100μF.
