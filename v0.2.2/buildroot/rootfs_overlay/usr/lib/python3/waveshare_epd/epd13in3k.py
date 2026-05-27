# *****************************************************************************
# * | File        :	  epd13in3k.py
# * | Author      :   Waveshare team
# * | Function    :   Electronic paper driver
# * | Info        :
# *----------------
# * | This version:   V1.0
# * | Date        :   2023-09-08
# # | Info        :   python demo
# -----------------------------------------------------------------------------
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documnetation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to  whom the Software is
# furished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in
# all copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS OR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
# THE SOFTWARE.
#


import logging
import os
import time
from . import epdconfig

# Display resolution
EPD_WIDTH       = 960
EPD_HEIGHT      = 680

GRAY1  = 0xff #white
GRAY2  = 0xC0
GRAY3  = 0x80 #gray
GRAY4  = 0x00 #Blackest

logger = logging.getLogger(__name__)

class EPD:
    def __init__(self):
        self.reset_pin = epdconfig.RST_PIN
        self.dc_pin = epdconfig.DC_PIN
        self.busy_pin = epdconfig.BUSY_PIN
        self.cs_pin = epdconfig.CS_PIN
        self.width = EPD_WIDTH
        self.height = EPD_HEIGHT
        self.GRAY1  = GRAY1 #white
        self.GRAY2  = GRAY2
        self.GRAY3  = GRAY3 #gray
        self.GRAY4  = GRAY4 #Blackest

        self.Lut_Partial=[									
            0x15,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x2A,	0x88,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x15,	0x44,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x00,	0x08,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x00,	0x01,	0x01,	0x01,	0x00,						
            0x0A,	0x00,	0x05,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x01,	0x01,						
            0x22,	0x22,	0x22,	0x22,	0x22,						
            0x17,	0x41,	0xA8,	0x32,	0x18,						
            0x00,	0x00,]

        self.LUT_DATA_4Gray =[											
            0x80,	0x48,	0x4A,	0x22,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x0A,	0x48,	0x68,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x88,	0x48,	0x60,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0xA8,	0x48,	0x45,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	0x00,	
            0x07,	0x23,	0x17,	0x02,	0x00,						
            0x05,	0x01,	0x05,	0x01,	0x02,						
            0x08,	0x02,	0x01,	0x04,	0x04,						
            0x00,	0x02,	0x00,	0x02,	0x01,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x00,						
            0x00,	0x00,	0x00,	0x00,	0x01,						
            0x22,	0x22,	0x22,	0x22,	0x22,						
            0x17,	0x41,	0xA8,	0x32,	0x30,						
            0x00,	0x00,]

        if (epdconfig.module_init() != 0):
            return -1
    
    # Hardware reset
    def reset(self):
        epdconfig.digital_write(self.reset_pin, 1)
        epdconfig.delay_ms(20) 
        epdconfig.digital_write(self.reset_pin, 0)
        epdconfig.delay_ms(2)
        epdconfig.digital_write(self.reset_pin, 1)
        epdconfig.delay_ms(20)   

    def send_command(self, command):
        epdconfig.digital_write(self.dc_pin, 0)
        epdconfig.digital_write(self.cs_pin, 0)
        epdconfig.spi_writebyte([command])
        epdconfig.digital_write(self.cs_pin, 1)

    def send_data(self, data):
        epdconfig.digital_write(self.dc_pin, 1)
        epdconfig.digital_write(self.cs_pin, 0)
        epdconfig.spi_writebyte([data])
        epdconfig.digital_write(self.cs_pin, 1)

    def send_data2(self, data):
        epdconfig.digital_write(self.dc_pin, 1)
        epdconfig.digital_write(self.cs_pin, 0)
        epdconfig.SPI.writebytes2(data)
        epdconfig.digital_write(self.cs_pin, 1)

    def ReadBusy(self):
        # Waveshare upstream waits while busy==1. Many SSD16xx panels drive BUSY **active-low**
        # (LOW while updating). If polarity is wrong, this returns immediately, TurnOnDisplay "finishes"
        # before the glass updates, and an immediate sleep()/PWR-off yields **no visible change**.
        logger.debug("e-Paper busy")
        invert = os.environ.get('SLOWOS_EINK_BUSY_INVERT', '').strip() == '1'
        timeout_s = float(os.environ.get('SLOWOS_EINK_BUSY_TIMEOUT_S', '180'))
        t0 = time.monotonic()
        while True:
            busy = epdconfig.digital_read(self.busy_pin)
            updating = (busy == 0) if invert else (busy == 1)
            if not updating:
                break
            if time.monotonic() - t0 > timeout_s:
                logger.warning(
                    'ReadBusy: timeout after %ss (BUSY disconnected or wrong polarity; '
                    'try SLOWOS_EINK_BUSY_INVERT=1)',
                    timeout_s,
                )
                break
            epdconfig.delay_ms(20)
        epdconfig.delay_ms(20)
        logger.debug("e-Paper busy release")

    def TurnOnDisplay(self):
        self.send_command(0x22) #Display Update Control
        self.send_data(0xF7)
        self.send_command(0x20) #Activate Display Update Sequence
        self.ReadBusy()
        # If BUSY GPIO is wrong/floating, ReadBusy returns instantly and the glass never finishes
        # before sleep/PWR-off. Optional pad (seconds) only when set — not used by eink-bridge default.
        pad = float(os.environ.get('SLOWOS_EINK_FULL_REFRESH_PAD_SEC', '0'))
        if pad > 0:
            time.sleep(pad)

    def TurnOnDisplay_Part(self):
        self.send_command(0x22) #Display Update Control
        self.send_data(0xCF)
        self.send_command(0x20) #Activate Display Update Sequence
        self.ReadBusy()

    def TurnOnDisplay_4GRAY(self):
        self.send_command(0x22) #Display Update Control
        self.send_data(0xC7)
        self.send_command(0x20) #Activate Display Update Sequence
        self.ReadBusy()

    def Lut(self, LUT):
        self.send_command(0x32)
        for i in range(105):
            self.send_data(LUT[i])

        self.send_command(0x03) 
        self.send_data(LUT[105])

        self.send_command(0x04)  
        self.send_data(LUT[106])
        self.send_data(LUT[107]) 
        self.send_data(LUT[108])

        self.send_command(0x2C)
        self.send_data(LUT[109])
        
    def init(self):
        
        # EPD hardware init start
        self.reset()
        self.ReadBusy()

        self.send_command(0x12) #SWRESET
        self.ReadBusy()

        self.send_command(0x0C) 
        self.send_data(0xAE)
        self.send_data(0xC7)  
        self.send_data(0xC3)
        self.send_data(0xC0)
        self.send_data(0x80)

        self.send_command(0x01) 
        self.send_data(0xA7)
        self.send_data(0x02)  
        self.send_data(0x00)

        self.send_command(0x11)
        self.send_data(0x03)

        self.send_command(0x44) 
        self.send_data(0x00)
        self.send_data(0x00)  
        self.send_data(0xBF)
        self.send_data(0x03) 
        
        self.send_command(0x45) 
        self.send_data(0x00)
        self.send_data(0x00)  
        self.send_data(0xA7)
        self.send_data(0x02)

        self.send_command(0x3C) 
        self.send_data(0x05)

        self.send_command(0x18) 
        self.send_data(0x80)

        self.send_command(0x4E) 
        self.send_data(0x00)
        self.send_data(0x00)

        self.send_command(0x4F) 
        self.send_data(0x00)
        self.send_data(0x00) 

        # EPD hardware init end
        return 0

    def init_Part(self):
        self.reset()

        self.send_command(0x3C)    
        self.send_data(0x80)

        self.Lut(self.Lut_Partial)

        self.send_command(0x37) 
        self.send_data(0x00)  
        self.send_data(0x00)  
        self.send_data(0x00)  
        self.send_data(0x00)  
        self.send_data(0x00) 
        self.send_data(0x40)  
        self.send_data(0x00)  
        self.send_data(0x00)   
        self.send_data(0x00)  
        self.send_data(0x00)  

        self.send_command(0x3C) 
        self.send_data(0x80)   

        self.send_command(0x22) 
        self.send_data(0xC0)   
        self.send_command(0x20) 

        self.ReadBusy()
    def init_4GRAY(self):
        self.reset()

        self.ReadBusy()   
        self.send_command(0x12)
        self.ReadBusy()   

        self.send_command(0x0C)   
        self.send_data(0xAE)
        self.send_data(0xC7)
        self.send_data(0xC3)
        self.send_data(0xC0)
        self.send_data(0x80)

        self.send_command(0x01) 
        self.send_data(0xA7)
        self.send_data(0x02)  
        self.send_data(0x00)

        self.send_command(0x11)
        self.send_data(0x03) 

        self.send_command(0x44) 
        self.send_data(0x00)
        self.send_data(0x00)  
        self.send_data(0xBF)
        self.send_data(0x03) 
        
        self.send_command(0x45) 
        self.send_data(0x00)
        self.send_data(0x00)  
        self.send_data(0xA7)
        self.send_data(0x02)

        self.send_command(0x3C)
        self.send_data(0x00)

        self.send_command(0x18) 
        self.send_data(0x80)
        
        self.send_command(0x4E) 
        self.send_data(0x00)
        self.send_data(0x00)

        self.send_command(0x4F) 
        self.send_data(0x00)
        self.send_data(0x00) 

        self.Lut(self.LUT_DATA_4Gray)
        
        self.ReadBusy()


    def getbuffer(self, image):
        # logger.debug("bufsiz = ",int(self.width/8) * self.height)
        buf = [0xFF] * (int(self.width / 8) * self.height)
        image_monocolor = image.convert('1')
        imwidth, imheight = image_monocolor.size
        pixels = image_monocolor.load()
        # logger.debug("imwidth = %d, imheight = %d",imwidth,imheight)
        if imwidth == self.width and imheight == self.height:
            logger.debug("Horizontal")
            for y in range(imheight):
                for x in range(imwidth):
                    # Set the bits for the column of pixels at the current position.
                    if pixels[x, y] == 0:
                        buf[int((x + y * self.width) / 8)] &= ~(0x80 >> (x % 8))
        elif imwidth == self.height and imheight == self.width:
            logger.debug("Vertical")
            for y in range(imheight):
                for x in range(imwidth):
                    newx = y
                    newy = self.height - x - 1
                    if pixels[x, y] == 0:
                        buf[int((newx + newy * self.width) / 8)] &= ~(0x80 >> (y % 8))
        return buf

    def getbuffer_4Gray(self, image):
        # logger.debug("bufsiz = ",int(self.width/8) * self.height)
        buf = [0xFF] * (int(self.width / 4) * self.height)
        image_monocolor = image.convert('L')
        imwidth, imheight = image_monocolor.size
        pixels = image_monocolor.load()
        i=0
        # logger.debug("imwidth = %d, imheight = %d",imwidth,imheight)
        if(imwidth == self.width and imheight == self.height):
            logger.debug("Vertical")
            for y in range(imheight):
                for x in range(imwidth):
                    # Set the bits for the column of pixels at the current position.
                    if(pixels[x, y] == 0xC0):
                        pixels[x, y] = 0x80
                    elif (pixels[x, y] == 0x80):
                        pixels[x, y] = 0x40
                    i= i+1
                    if(i%4 == 0):
                        buf[int((x + (y * self.width))/4)] = ((pixels[x-3, y]&0xc0) | (pixels[x-2, y]&0xc0)>>2 | (pixels[x-1, y]&0xc0)>>4 | (pixels[x, y]&0xc0)>>6)
                        
        elif(imwidth == self.height and imheight == self.width):
            logger.debug("Horizontal")
            for x in range(imwidth):
                for y in range(imheight):
                    newx = y
                    newy = self.height - x - 1
                    if(pixels[x, y] == 0xC0):
                        pixels[x, y] = 0x80
                    elif (pixels[x, y] == 0x80):
                        pixels[x, y] = 0x40
                    i= i+1
                    if(i%4 == 0):
                        buf[int((newx + (newy * self.width))/4)] = ((pixels[x, y-3]&0xc0) | (pixels[x, y-2]&0xc0)>>2 | (pixels[x, y-1]&0xc0)>>4 | (pixels[x, y]&0xc0)>>6) 
        return buf

    def Clear(self):
        buf = [0xFF] * (int(self.width/8) * self.height)
        self.send_command(0x24)
        self.send_data2(buf)

        self.TurnOnDisplay()
    
    def display(self, image):
        self.send_command(0x24)
        self.send_data2(image)

        self.TurnOnDisplay()

    def display_Base(self, image):
        """
        Loads the monochrome frame into SRAM used as the baseline for LUT-driven partial refresh.

        This is **SSD16xx/JD79686-class** sequencing (dual bank 0x24 + 0x26), **not IT8951** image
        buffer DMA. Vendor C (`EPD_13IN3K_Display_Base`) mirrors the same mono payload into **both**
        command streams so subsequent `TurnOnDisplay_Part()` partial pulses have a coherent
        “previous” reference at the chip. SlowOS bridges must restore this baseline after long
        sleep or any detected cache/controller mismatch (`init`/`display_Base`/`init_Part`).

        Separate “CPU-side previous buffer restore before partial” hacks known from some OTHER
        Waveshare controller families **do not apply** mechanically here; correctness is guarded
        in `slowos`'s displayed-model `_last_push_l`, not duplicate SPI rewinds besides this call.
        """
        self.send_command(0x24)
        self.send_data2(image)

        self.send_command(0x26)
        self.send_data2(image)

        self.TurnOnDisplay()

    def display_Base_color(self, color):
        if(self.width % 8 == 0):
            Width = self.width // 8
        else:
            Width = self.width // 8 +1
        Height = self.height
        self.send_command(0x24)   #Write Black and White image to RAM
        for j in range(Height):
            for i in range(Width):
                self.send_data(color)
                
        self.send_command(0x26)  #Write Black and White image to RAM
        for j in range(Height):
            for i in range(Width):
                self.send_data(color)
        # self.TurnOnDisplay()

    def display_Partial(self, Image, Xstart, Ystart, Xend, Yend):
        """
        Partial refresh — SlowOS: same convention as Waveshare C EPD_13IN3K_Display_Part
        (EPD_13in3k.c): (Xstart,Ystart) top-left pixel; Xend/Yend are **exclusive**
        right/bottom edges (same as passing w,h via x+w, y+l). Image is the full
        getbuffer() mono buffer (width/8 * height bytes).

        Upstream Python used bitwise |/& in the alignment test and wrong X high-byte
        encoding ((>>5)&1 vs C's (pixel>>8)&3), and streamed a sparse subset of bytes;
        the controller expects a contiguous (width_bytes * height_px) payload after
        SetWindows/SetCursor, which produced top-left misplacement on partial updates.
        """
        x = int(Xstart)
        y = int(Ystart)
        w = int(Xend) - x
        l = int(Yend) - y
        if w <= 0 or l <= 0:
            return

        Xs = x
        Xe = x + w
        y0 = y
        y1 = y + l

        if (
            (Xs % 8 + Xe % 8 == 8 and Xs % 8 > Xe % 8)
            or (Xs % 8 + Xe % 8 == 0)
            or ((Xe - Xs) % 8 == 0)
        ):
            xs_b = Xs // 8
            xe_b = Xe // 8
        else:
            xs_b = Xs // 8
            xe_b = (Xe // 8) if (Xe % 8 == 0) else (Xe // 8 + 1)

        width_bytes = xe_b - xs_b
        height_px = y1 - y0
        image_counter = width_bytes * height_px

        xe_incl = xe_b - 1
        ye_incl = y1 - 1

        xs_px = xs_b * 8
        xe_px = xe_incl * 8

        self.send_command(0x44)
        self.send_data(xs_px & 0xFF)
        self.send_data((xs_px >> 8) & 0x03)
        self.send_data(xe_px & 0xFF)
        self.send_data((xe_px >> 8) & 0x03)

        self.send_command(0x45)
        self.send_data(y0 & 0xFF)
        self.send_data((y0 >> 8) & 0x03)
        self.send_data(ye_incl & 0xFF)
        self.send_data((ye_incl >> 8) & 0x03)

        self.send_command(0x4E)
        self.send_data(xs_px & 0xFF)
        self.send_data((xs_px >> 8) & 0x03)
        self.send_command(0x4F)
        self.send_data(y0 & 0xFF)
        self.send_data((y0 >> 8) & 0x03)

        full_w = self.width // 8
        chunk = []
        for row in range(y0, y1):
            base = xs_b + row * full_w
            chunk.extend(Image[base : base + width_bytes])

        if len(chunk) != image_counter:
            logger.warning(
                'display_Partial byte count mismatch: got %d expected %d',
                len(chunk),
                image_counter,
            )

        self.send_command(0x24)
        self.send_data2(bytes(chunk))
        self.TurnOnDisplay_Part()
    
    def display_4Gray(self, image):
        self.send_command(0x24)
        for i in range(0, 81600): 
            temp3=0
            for j in range(0, 2):
                temp1 = image[i*2+j]
                for k in range(0, 2):
                    temp2 = temp1&0xC0 
                    if(temp2 == 0xC0):
                        temp3 |= 0x00
                    elif(temp2 == 0x00):
                        temp3 |= 0x01  
                    elif(temp2 == 0x80): 
                        temp3 |= 0x01 
                    else: #0x40
                        temp3 |= 0x00 
                    temp3 <<= 1	
                    
                    temp1 <<= 2
                    temp2 = temp1&0xC0 
                    if(temp2 == 0xC0): 
                        temp3 |= 0x00
                    elif(temp2 == 0x00): 
                        temp3 |= 0x01
                    elif(temp2 == 0x80):
                        temp3 |= 0x01
                    else :   #0x40
                        temp3 |= 0x00	
                    if(j!=1 or k!=1):				
                        temp3 <<= 1
                    temp1 <<= 2
            self.send_data(temp3)
            
        self.send_command(0x26)	       
        for i in range(0, 81600): 
            temp3=0
            for j in range(0, 2):
                temp1 = image[i*2+j]
                for k in range(0, 2):
                    temp2 = temp1&0xC0 
                    if(temp2 == 0xC0):
                        temp3 |= 0x00
                    elif(temp2 == 0x00):
                        temp3 |= 0x01
                    elif(temp2 == 0x80):
                        temp3 |= 0x00
                    else: #0x40
                        temp3 |= 0x01 
                    temp3 <<= 1	
                    
                    temp1 <<= 2
                    temp2 = temp1&0xC0 
                    if(temp2 == 0xC0): 
                        temp3 |= 0x00
                    elif(temp2 == 0x00): 
                        temp3 |= 0x01
                    elif(temp2 == 0x80):
                        temp3 |= 0x00 
                    else:    #0x40
                            temp3 |= 0x01	
                    if(j!=1 or k!=1):					
                        temp3 <<= 1
                    temp1 <<= 2
            self.send_data(temp3)
        
        self.TurnOnDisplay_4GRAY()


    def sleep(self):
        self.send_command(0x10) # DEEP_SLEEP
        self.send_data(0x03)
        
        epdconfig.delay_ms(2000)
        epdconfig.module_exit()
### END OF FILE ###
