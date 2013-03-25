from ..utils import write_yaml


def print_configuration_help(ns):
    print ns.term.bold_red(
        "Set a value to the 'api_key' option in {0}".format(ns.config_path))


def action(ns):
    print ns.term.bold_magenta(u"Synchronizing local database")
    print u""

    if not ns.api:
        print ns.term.bold_red("API not configured")
        print_configuration_help(ns)
        return 1

    if not ns.mirrors:
        print ns.term.bold_green("Downloading mirrors to {0}".format(
            ns.mirrors_path))

        with open(ns.mirrors_path, 'w') as fp:
            write_yaml(fp, ns.api.mirrors())

    if not ns.languages:
        with open(ns.languages_path, 'w') as fp:
            write_yaml(fp, ns.api.languages())

    if ns.language:
        for series in ns.series.list_series():
            print ns.term.bold_cyan(
                u"syncing: {0}".format(series['series_name']))

            series, episodes = ns.api.series_all(
                series['id'], ns.language)

            ns.series.set_episodes(series, episodes)

    return 0


def setup(parser):
    parser.set_defaults(action=action)
