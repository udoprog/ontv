import requests
from lxml import etree

DEFAULT_URL = "http://thetvdb.com"


def text_value(element_type):
    def __inner(c):
        if not c.text:
            return None
        return element_type(c.text)

    return __inner


def text_list(element_type, separator="|"):
    """
    Parse a list contained withing a text field.
    """

    def __inner(c):
        if not c.text:
            return None

        results = list()

        for item in c.text.split(separator):
            item = item.strip()

            if not item:
                continue

            results.append(element_type(item))

        return results

    return __inner


class TheTVDBApi(object):
    MIRROR = {
        "id": ("id", text_value(int)),
        "mirrorpath": ("mirrorpath", text_value(unicode)),
        "typemask": ("typemask", text_value(int)),
    }

    LANGUAGE = {
        "id": ("id", text_value(int)),
        "abbreviation": ("abbreviation", text_value(unicode)),
        "name": ("name", text_value(unicode)),
    }

    SERIES = {
        "id": ("id", text_value(int)),
        "seriesid": ("series_id", text_value(int)),
        "language": ("language", text_value(unicode)),
        "SeriesName": ("series_name", text_value(unicode)),
        "Overview": ("overview", text_value(unicode)),
        "FirstAired": ("first_aired", text_value(unicode)),
    }

    BASE_SERIES = {
        "id": ("id", text_value(int)),
        "Actors": ("actors", text_list(unicode)),
        "Airs_DayOfWeek": ("airs_day_of_week", text_value(unicode)),
        "Airs_Time": ("airs_time", text_value(unicode)),
        "FirstAired": ("first_aired", text_value(unicode)),
        "Genre": ("genre", text_list(unicode)),
        "IMDB_ID": ("imdb_id", text_value(unicode)),
        "Language": ("language", text_value(unicode)),
        "Network": ("network", text_value(unicode)),
        "NetworkID": ("network_id", text_value(int)),
        "Overview": ("overview", text_value(unicode)),
        "Rating": ("rating", text_value(float)),
        "Runtime": ("runtime", text_value(int)),
        "SeriesID": ("series_id", text_value(str)),
        "SeriesName": ("series_name", text_value(unicode)),
        "Status": ("status", text_value(unicode)),
        "added": ("added", text_value(unicode)),
        "addedBy": ("added_by", text_value(unicode)),
        "lastupdated": ("lastupdated", text_value(int)),
    }

    EPISODE = {
        "id": ("id", text_value(int)),
        "EpisodeNumber": ("episode_number", text_value(int)),
        "EpisodeName": ("episode_name", text_value(unicode)),
        "SeasonNumber": ("season_number", text_value(int)),
        "Overview": ("overview", text_value(unicode)),
        "FirstAired": ("first_aired", text_value(unicode)),
        "GuestStars": ("guest_stars", text_list(unicode)),
        "Writer": ("writer", text_list(unicode)),
        "seasonid": ("season_id", text_value(int)),
        "seriesid": ("series_id", text_value(str)),
        "lastupdated": ("lastupdated", text_value(int)),
    }

    def __init__(self, api_key, base_url=None):
        if base_url is None:
            base_url = DEFAULT_URL

        if api_key:
            self.api_url = "{0}/api/{1}".format(base_url, api_key)
        else:
            self.api_url = None

        self.open_api_url = "{0}/api".format(base_url)
        self.session = requests.session()

    def _request(self, method, base_url, path, **kw):
        if base_url is None:
            raise Exception("Invalid base url")

        url = "{0}/{1}".format(base_url, path)

        r = self.session.get(url, **kw)

        if not r.ok:
            raise Exception("request failed: {0} ({1})".format(
                url, r.status_code))

        return etree.XML(r.content)

    def is_authenticated(self):
        return self.api_url is not None

    def _assert_api_key(self):
        if self.api_url is None:
            raise Exception("No API key specified")

    def get(self, path, **kw):
        self._assert_api_key()
        return self._request('GET', self.api_url, path, **kw)

    def open_get(self, path, **kw):
        return self._request('GET', self.open_api_url, path, **kw)

    def _parse_element(self, element, types):
        result = dict()

        for key, _ in types.values():
            result[key] = None

        for c in element:
            element_specifier = types.get(c.tag)

            if element_specifier is None:
                continue

            target_key, element_type = element_specifier

            try:
                result[target_key] = element_type(c)
            except Exception as e:
                raise Exception(
                    "Failed to parse field '{0}': {1}".format(
                        c.tag, str(e)))

        return result

    def mirrors(self):
        document = self.get('mirrors.xml')
        return [self._parse_element(child, self.MIRROR)
                for child in document.iter("Mirror")]

    def languages(self):
        document = self.get('languages.xml')
        return [self._parse_element(child, self.LANGUAGE)
                for child in document.iter("Language")]

    def getseries(self, seriesname):
        document = self.open_get(
            'GetSeries.php',
            params={"seriesname": seriesname})

        return [self._parse_element(child, self.SERIES)
                for child in document.iter("Series")]

    def series(self, series_id, language):
        document = self.get(
            'series/{0}/{1}.xml'.format(series_id, language))

        series = document.find("Series")

        if not len(series):
            return None

        return self._parse_element(series, self.BASE_SERIES)

    def series_all(self, series_id, language):
        document = self.get(
            'series/{0}/all/{1}.xml'.format(series_id, language))

        series = document.find("Series")
        episodes = document.findall("Episode")

        if len(series):
            series = self._parse_element(series, self.BASE_SERIES)
        else:
            series = None

        episodes = [self._parse_element(episode, self.EPISODE)
                    for episode in episodes]

        return series, episodes
