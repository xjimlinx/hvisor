// SPDX-License-Identifier: GPL-2.0
/*
 * OK8MP-C GPIO SuperSpeed orientation switch, ported to current Type-C API.
 * Based on NXP gpio-switch.c, Copyright 2019 NXP, author Jun Li.
 * Keep the vendor compatible and polarity; this is not an SBU/DP mux.
 */
#include <linux/gpio/consumer.h>
#include <linux/module.h>
#include <linux/mutex.h>
#include <linux/platform_device.h>
#include <linux/usb/typec_mux.h>

struct ok8mp_switch {
	struct gpio_desc *select;
	struct typec_switch_dev *sw;
	struct mutex lock;
};

static int ok8mp_switch_set(struct typec_switch_dev *sw,
			   enum typec_orientation orientation)
{
	struct ok8mp_switch *s = typec_switch_get_drvdata(sw);

	mutex_lock(&s->lock);
	/* Vendor GPIO_ACTIVE_LOW: normal = physical low, reverse = high. */
	if (orientation == TYPEC_ORIENTATION_NORMAL)
		gpiod_set_value_cansleep(s->select, 1);
	else if (orientation == TYPEC_ORIENTATION_REVERSE)
		gpiod_set_value_cansleep(s->select, 0);
	mutex_unlock(&s->lock);
	return 0;
}

static int ok8mp_switch_probe(struct platform_device *pdev)
{
	struct device *dev = &pdev->dev;
	struct typec_switch_desc desc = { };
	struct ok8mp_switch *s;

	s = devm_kzalloc(dev, sizeof(*s), GFP_KERNEL);
	if (!s)
		return -ENOMEM;
	mutex_init(&s->lock);
	s->select = devm_gpiod_get(dev, "switch", GPIOD_OUT_LOW);
	if (IS_ERR(s->select))
		return dev_err_probe(dev, PTR_ERR(s->select), "select GPIO unavailable\n");
	desc.drvdata = s;
	desc.fwnode = dev_fwnode(dev);
	desc.set = ok8mp_switch_set;
	s->sw = typec_switch_register(dev, &desc);
	if (IS_ERR(s->sw))
		return dev_err_probe(dev, PTR_ERR(s->sw), "switch registration failed\n");
	platform_set_drvdata(pdev, s);
	return 0;
}

static void ok8mp_switch_remove(struct platform_device *pdev)
{
	struct ok8mp_switch *s = platform_get_drvdata(pdev);

	typec_switch_unregister(s->sw);
}

static const struct of_device_id ok8mp_switch_match[] = {
	{ .compatible = "nxp,cbtl04gp" },
	{ }
};
MODULE_DEVICE_TABLE(of, ok8mp_switch_match);

static struct platform_driver ok8mp_switch_driver = {
	.probe = ok8mp_switch_probe,
	.remove = ok8mp_switch_remove,
	.driver = {
		.name = "ok8mp-typec-switch",
		.of_match_table = ok8mp_switch_match,
	},
};
module_platform_driver(ok8mp_switch_driver);
MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("Forlinx OK8MP-C GPIO USB-C SuperSpeed orientation switch");
