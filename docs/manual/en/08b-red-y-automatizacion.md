# Network connection and automation

How to connect your machine over Wi-Fi or Ethernet instead of a USB cable, how to test the connection before using it, how to rehearse without a machine using a virtual machine over the network, and how to let other programs on your own computer drive Tlaku with UDP commands.

> **Be aware, in all honesty:** the network connection and UDP automation **have not been tested with a real machine**. They were validated with the `tlaku-emu` emulator behind real sockets (TCP and WebSocket) and with automated tests. (What was tested on a real machine, a Falcon2 Pro over USB, is the serial connection; see [Supported controllers](13-controladoras.md).) Do your first test with the laser at low power and without leaving the machine unattended. Which controllers work and in what state: [Supported controllers](13-controladoras.md).

## Which machines connect over the network

| Machine | Connection type in the profile | Typical address |
|---|---|---|
| grblHAL with Ethernet | **Network · TCP / Telnet** | The machine's IP and port 23 |
| FluidNC (Telnet server) | **Network · TCP / Telnet** | The IP or name (for example `fluidnc.local`) and port 23 |
| FluidNC or ESP3D (WebSocket) | **Network · WebSocket** | `ws://192.168.1.50:81`; the port is usually 81 |
| Ruida controller (CO2) | Its own, over UDP | See [Ruida and K40](13b-ruida-y-k40.md) |

What does **not** exist: encrypted connections (`wss://`), the proprietary networks of xTool or Glowforge, Trocen and TopWisdom, or automatic machine discovery on the network (you type the address by hand).

## Setting up a network machine

The connection is a **profile** setting, not a port setting. Example: a FluidNC with IP `192.168.1.50` that listens over Telnet.

1. Open the **Machine** menu, **Manage Devices…**, and choose or create the profile (for example, with the **FluidNC** controller type).
2. In the **Communication** group, open **Connection type** and choose **Network · TCP / Telnet**.
3. In **Machine address** type `192.168.1.50` (without `tcp://`, spaces, or slashes) and leave the **Port** at `23`.
4. Press **Test connection** (below) and, if it answers, **Save changes**.
5. Go to the **Machine** tab. In the **Connection** section the serial port and baud rate are gone: you see the **Address** (`192.168.1.50:23`) and a note saying how to change it. Press **Connect**.

If your machine uses WebSocket, choose **Network · WebSocket** and type the full **WebSocket address**, for example `ws://192.168.1.50:81`. Switching between TCP and WebSocket keeps the machine name (`192.168.1.50`); you only need to check the port.

The screen corrects you before saving:

- **Empty address, or one with `tcp://`, spaces, or slashes:** "Enter the machine address, without "tcp://", spaces or slashes."
- **Port outside 1 to 65535:** "The port must be a whole number from 1 to 65535."
- **WebSocket that does not start with `ws://`:** it asks you to write it that way; with `wss://` it tells you encrypted connections are not supported.

> **Tip:** if your router does not resolve names like `fluidnc.local`, use the IP. It is a good idea to reserve a fixed IP for the machine in the router so it does not change.

## Testing the connection

**Test connection** (in the same **Communication** group) opens the address, reads the machine's greeting, makes the same read-only queries as detection, and closes. **It moves nothing and changes no setting**, so it is safe to do with the machine on and the lid closed.

- If all goes well you read "The machine answered: Grbl 1.1h" (or whatever firmware it reports). If it answers but the firmware is not recognized, it says so.
- If it fails, the message says what happened and what to check (see the next table).
- It is disabled while the address is not valid and when you are already connected to that same address (disconnect first).
- Changing the address clears the previous result: the test is valid only for the address you see.

## If the connection fails

Errors come explained in plain language, with the address that was tried:

| Message | What it means and what to do |
|---|---|
| "'X' was not found on the network" | The name does not resolve. Check how it is spelled or use the IP |
| "The machine at X refused the connection" | The port is closed or the controller's network service is off. Telnet usually uses 23; check the port and the board's configuration |
| "X did not answer in time" | There is a 5-second timeout. The machine is off, on another network, or the IP is different |
| "X cannot be reached" | The device is not reachable from this network. Check the Wi-Fi or cable and the IP |
| "X closed the connection when it was opened: is another program connected to it?" | Many boards accept only one client. Close the other program (the browser with the web interface, other laser software) |
| "X closed the network connection" (during the job) | The network was lost. The app reports it as a disconnection; see [Troubleshooting](15-solucion-de-problemas.md) |

## What changes compared to USB

- **The board does not reset on connect**, so it connects faster; Tlaku only discards a short greeting (up to 1.5 seconds) before asking for the firmware.
- **There is no baud rate.** The selector disappears.
- Everything else is the same: the console, framing, the job, pause, and live adjustments use the same path as with the cable. See [Sending jobs safely](09-enviar-con-seguridad.md).
- A network connection can drop without anyone touching a cable (the Wi-Fi goes down). If that happens mid-job, the machine may keep running what it already had in its buffer: keep the emergency stop within reach.

## Virtual machine over the network

To learn the workflow without having the machine, the **Port** list of a profile with a **Serial port (USB)** connection offers, besides the usual virtual machines, the **Virtual machine over network · Falcon2 Pro**, **GRBL 1.1**, **grblHAL**, **FluidNC**, **Marlin**, and **Smoothieware**.

Unlike the normal virtual machine, this one is an emulator that listens on a local TCP port on your own computer (`127.0.0.1`), and the app connects to it **for real, through a socket**, as it would with a network machine. It lets you see how the network connection behaves (greeting, job, disconnection) without hardware. The laser is still simulated: nothing is engraved. When you disconnect, the socket is closed and the emulator stops. It has the same speeds (`×1`, `×5`, `×20`) and the **Simulate fault** section of the normal virtual machine (see [Connecting your machine](08-conectar-maquina.md)).

## UDP automation

Tlaku can **listen for text commands** sent by other programs or scripts **on your own computer**: open a file, ask for the machine's status, stop a job, or ask it to start. It is useful, for example, for a script in your shop (or a production program) to leave a design loaded, for a physical button to stop the laser, or for a spreadsheet to check whether the machine is free.

**It is off by default.** To turn it on: **Edit > Preferences…** (`Ctrl/Cmd+,`), **Automation** section.

1. Tick **Turn on UDP automation**. The **UDP port** appears (47801 by default; 1024 to 65535) along with a status: "Listening on 127.0.0.1:47801". If the port is busy, it says it could not be turned on and why; change the port.
2. Leave **Allow starting from automation** off, unless you know very well what you are doing (see below).

### Security

- **It only listens on `127.0.0.1`**: no other computer on your network can send it anything. But **any program running on your computer** can, so do not turn it on on a shared computer with software you do not control.
- **`START` does not fire the laser by itself.** With the checkbox off (the default), `START` only opens the usual Start dialog, with its checklist, and **you decide**. The reply is `PENDING`.
- With **Allow starting from automation** on, `START` starts the job without asking: any program on this computer can start the laser. Even so, if you have the **Before sending** checklist in the Start dialog set as required, the dialog asks. And it is always required that the machine be connected and idle, and that there be something to send.
- **`STOP` is always honored** if a machine is connected: stopping is never dangerous.
- **`LOAD` only opens files that exist**, with an absolute path, using the same importer as always. There is no way to read or write files from here.

### The protocol

One command per UDP datagram, as a single line of UTF-8 text (up to 4096 bytes). Capitalization does not matter. The reply is a one-line datagram, sent to whoever asked.

| Command | Reply | What it does |
|---|---|---|
| `PING` | `PONG` | Checks that Tlaku is listening |
| `LOAD /absolute/path/piece.svg` | `OK loading` | Opens a `.tlaku` project in a new tab or imports the design into the active tab, as if you dropped it onto the window. The path may contain spaces |
| `START` | `OK starting` or `PENDING confirm_in_app: ...` | Asks to start the job (see Security) |
| `STATUS` | `STATUS connected=1 state=idle x=12.500 y=30.000` | Machine state and its work position in mm; with no machine, `STATUS connected=0` |
| `STOP` | `OK` | Stops the job (feed hold and soft reset); with no job, cancels the movement |

Errors arrive as `ERR <code>: <text in the app's language>`; the codes are `not_connected`, `busy`, `not_found`, `bad_argument`, `bad_text`, `too_long`, `empty`, `unknown_command`, and `failed`.

> **Note:** this protocol is Tlaku's own. **It is not LightBurn's**: scripts made for LightBurn do not work without changes.

### Examples

From a terminal on macOS or Linux (with `nc`):

```sh
echo PING | nc -u -w1 127.0.0.1 47801
echo 'LOAD /Users/ana/disenos/llavero.svg' | nc -u -w1 127.0.0.1 47801
echo STATUS | nc -u -w1 127.0.0.1 47801
```

From Python, a script that loads a design and waits for the machine to be free before asking it to start:

```python
import socket, time

def tlaku(cmd, port=47801):
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.settimeout(2)
    s.sendto(cmd.encode(), ("127.0.0.1", port))
    return s.recvfrom(4096)[0].decode().strip()

print(tlaku("LOAD /Users/ana/disenos/llavero.svg"))   # OK loading
while "state=idle" not in tlaku("STATUS"):
    time.sleep(1)
print(tlaku("START"))   # PENDING confirm_in_app: ... -> you confirm in the window
```

With the checkbox off, that `START` leaves the Start dialog open in the window and you confirm with the preview in front of you. It is the recommended way to work.

The technical detail is in `docs/app-api/automatizacion.md`, section "Automatización por UDP" (the API docs are in Spanish).

Next: [Sending jobs safely](09-enviar-con-seguridad.md)
